#!/usr/bin/env bash
# Prueft `postinst` und `postrm` des Pakets mit Attrappen (HUM-284).
#
# Aufruf: packaging/deb/maintscripts-test.sh
#
# Die Skripte werden so zusammengesetzt wie fuer das Paket
# (`maintscripts.sh`) und dann gegen Attrappen von `loginctl`, `systemctl`,
# `getent`, `setsid`, `setpriv`, `logger`, `systemd-detect-virt`,
# `policy-rc.d` und der Kommandozeile gefahren, die vorn im `PATH` liegen.
# Kein Fall erreicht den systemd, die Nutzer oder den Daemon des Rechners:
# `setpriv` fuehrt nie aus, was es bekommt, sondern reicht es an die Attrappe
# gleichen Namens weiter und schreibt mit, als wer. `timeout` ist das echte,
# davor eine Attrappe, die mitschreibt, womit es gerufen wurde.
#
# Faelle: Ein Daemon ohne Sitzung verabschiedet sich auf Bitte und wird nach
# `daemon-reload` neu gestartet; einer mit Sitzung, mit unlesbarer Antwort,
# ein gestoppter und einer aus einer eigenen Unit nicht; ein Daemon vor
# Vertrag 1.19 geht den alten Weg ueber `daemon status`; jeder Aufruf als
# Mensch steht unter `timeout -k 5` ausserhalb von `setsid` und `setpriv`,
# ohne Terminal und mit stdin `/dev/null`; ein haengendes `systemctl` haelt
# `postinst` nicht auf; ein scheiterndes `daemon-reload` oder `restart`
# bricht nichts ab; `policy-rc.d` mit 101, ein chroot und ein Rechner ohne
# systemd lassen alles aus; `postrm remove` laedt nur neu. Liegt `shellcheck`
# im `PATH` (die CI-Laeufer haben es), prueft es beide Skripte als POSIX-sh.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

scripts="$work/scripts"
"$here/maintscripts.sh" "$scripts"

stubs="$work/stubs"
state="$work/state"
systemd_dir="$work/run-systemd-system"
mkdir -p "$stubs" "$state" "$systemd_dir" "$work/tmp"
real_timeout="$(command -v timeout)"
# stdin der Laeufe: eine gewoehnliche Datei, nicht /dev/null. Nur so faellt
# ein Aufruf auf, der stdin erbt, statt es auf /dev/null zu setzen.
: >"$work/stdin"

# --- Attrappen ---------------------------------------------------------------

cat >"$stubs/loginctl" <<'EOF'
#!/bin/sh
echo "loginctl $*" >>"$STUB_STATE/log"
[ "$1 $2" = "list-users --no-legend" ] || exit 1
cat "$STUB_STATE/users"
EOF

# `systemctl` des Systems: nur die Frage nach der Nutzerinstanz.
cat >"$stubs/systemctl" <<'EOF'
#!/bin/sh
echo "system: systemctl $*" >>"$STUB_STATE/log"
case "$*" in
  "is-active --quiet user@"*".service")
    uid=${3#user@}
    uid=${uid%.service}
    [ -e "$STUB_STATE/manager-$uid" ]
    exit $?
    ;;
esac
exit 1
EOF

cat >"$stubs/getent" <<'EOF'
#!/bin/sh
[ "$1" = passwd ] || exit 2
name=$(sed -n "s/^$2 //p" "$STUB_STATE/users")
[ -n "$name" ] || exit 2
echo "$name:x:$2:$(( $2 + 100 ))::/home/$name:/bin/sh"
EOF

cat >"$stubs/logger" <<'EOF'
#!/bin/sh
echo "logger $*" >>"$STUB_STATE/log"
EOF

cat >"$stubs/systemd-detect-virt" <<'EOF'
#!/bin/sh
[ "$*" = "--chroot --quiet" ] || exit 2
[ -e "$STUB_STATE/chroot" ]
EOF

cat >"$stubs/policy-rc.d" <<'EOF'
#!/bin/sh
echo "policy-rc.d $*" >>"$STUB_STATE/log"
[ -e "$STUB_STATE/policy-deny" ] && exit 101
exit 0
EOF

# `timeout`: schreibt seine Argumente mit und ist dann das echte.
cat >"$stubs/timeout" <<EOF
#!/bin/sh
echo "timeout \$*" >>"\$STUB_STATE/log"
exec "$real_timeout" "\$@"
EOF

# `setsid`: schreibt mit, woher stdin kommt und ob stdout oder stderr ein
# Terminal ist, und reicht weiter, ohne eine Sitzung anzulegen.
cat >"$stubs/setsid" <<'EOF'
#!/bin/sh
in=$(readlink /proc/$$/fd/0)
tty=no
if [ -t 0 ] || [ -t 1 ] || [ -t 2 ]; then tty=yes; fi
out=file
[ -f "$(readlink /proc/$$/fd/1)" ] || out=other
echo "setsid stdin=$in stdout=$out tty=$tty next=$1" >>"$STUB_STATE/log"
exec "$@"
EOF

# `setpriv` liest die Optionen und das `env -i` mit seinen Zuweisungen und
# reicht den Rest an `user-<name>` weiter. Ausgefuehrt wird nie das Programm
# selbst.
cat >"$stubs/setpriv" <<'EOF'
#!/bin/sh
uid= gid= groups=no
while [ $# -gt 0 ]; do
  case "$1" in
    --reuid=*) uid=${1#--reuid=} ;;
    --regid=*) gid=${1#--regid=} ;;
    --init-groups) groups=yes ;;
    --*) echo "setpriv: unexpected option $1" >>"$STUB_STATE/log"; exit 97 ;;
    *) break ;;
  esac
  shift
done
[ "$1" = env ] && [ "$2" = -i ] || { echo "setpriv: no env -i" >>"$STUB_STATE/log"; exit 97; }
shift 2
env=
while [ $# -gt 0 ]; do
  case "$1" in
    *=*) env="$env $1"; shift ;;
    *) break ;;
  esac
done
name=$(basename "$1")
shift
echo "as $uid:$gid groups=$groups$env: $name $*" >>"$STUB_STATE/log"
STUB_UID=$uid exec "$STUB_DIR/user-$name" "$@"
EOF

# `systemctl --user` eines Menschen. `hang-<uid>` laesst `is-active` haengen.
cat >"$stubs/user-systemctl" <<'EOF'
#!/bin/sh
[ "$1" = --user ] || exit 98
shift
u=$STUB_UID
case "$1" in
  daemon-reload) [ ! -e "$STUB_STATE/reload-fails-$u" ] ;;
  is-active)
    [ -e "$STUB_STATE/hang-$u" ] && exec sleep 300
    [ -e "$STUB_STATE/active-$u" ]
    ;;
  show)
    if [ -e "$STUB_STATE/fragment-$u" ]; then
      cat "$STUB_STATE/fragment-$u"
    else
      echo /usr/lib/systemd/user/humanitld.service
    fi
    ;;
  restart | try-restart) [ ! -e "$STUB_STATE/restart-fails-$u" ] ;;
  *) exit 99 ;;
esac
EOF

# Die Kommandozeile eines Menschen. `retire-if-idle` antwortet mit dem
# Ausgang aus `retire-<uid>` (ohne die Datei: kein Daemon antwortet),
# `daemon status` mit der Sitzung aus `session-<uid>`.
cat >"$stubs/user-humanitl" <<'EOF'
#!/bin/sh
case "$*" in
  "--json daemon retire-if-idle")
    [ -e "$STUB_STATE/retire-$STUB_UID" ] || exit 2
    printf '{"outcome":"%s"}\n' "$(cat "$STUB_STATE/retire-$STUB_UID")"
    ;;
  "--json daemon status")
    [ -e "$STUB_STATE/session-$STUB_UID" ] || exit 2
    printf '{"agent_session":"%s","daemon_version":"0.0.6","unit":"active"}\n' \
      "$(cat "$STUB_STATE/session-$STUB_UID")"
    ;;
  *) exit 98 ;;
esac
EOF
chmod 0755 "$stubs"/*

# --- Werkzeug ----------------------------------------------------------------

failures=0
fail() {
  echo "FAIL: $*" >&2
  failures=$((failures + 1))
}

# Setzt den Zustand zurueck: Menschen als "uid name"-Zeilen.
reset_state() {
  rm -rf "$state"
  mkdir -p "$state"
  : >"$state/log"
  printf '%s\n' "$@" >"$state/users"
}

# run <script> <args...>: faehrt das Skript mit den Attrappen; Exit in $rc.
# STEP_SECONDS und SYSTEMD_DIR lassen sich je Fall setzen.
run() {
  local script="$1"
  shift
  rc=0
  env -i PATH="$stubs:/usr/bin:/bin" STUB_STATE="$state" STUB_DIR="$stubs" TMPDIR=/nonexistent HUMANITL_OUT_DIR="$work/tmp" \
    HUMANITL_SYSTEMD_DIR="${SYSTEMD_DIR:-$systemd_dir}" \
    HUMANITL_STEP_SECONDS="${STEP_SECONDS:-60}" \
    sh "$scripts/$script" "$@" <"$work/stdin" >"$state/out" 2>"$state/err" || rc=$?
}

has() {
  grep -qF -- "$1" "$state/log" || fail "$2: the log lacks '$1'"
}

lacks() {
  if grep -qF -- "$1" "$state/log"; then
    fail "$2: the log must not contain '$1'"
  fi
}

said() {
  grep -qF -- "$1" "$state/err" || fail "$2: stderr lacks '$1'"
}

# Die Zeile, in der $1 zuerst steht.
line_of() {
  { grep -nF -- "$1" "$state/log" || true; } | head -n 1 | cut -d: -f1
}

# --- Faelle ------------------------------------------------------------------

# 1. Sechs Menschen mit laufender Nutzerinstanz, einer ohne.
reset_state "1000 alice" "1001 bob" "1002 carol" "1003 dave" "1004 erin" "1005 frank" "1006 gina"
for uid in 1000 1001 1002 1003 1004 1006; do : >"$state/manager-$uid"; done
for uid in 1000 1001 1002 1004 1006; do : >"$state/active-$uid"; done
echo retired >"$state/retire-1000"
echo busy >"$state/retire-1001"
# carol: kein retire-1002, ihr Daemon antwortet nicht.
# dave: kein active-1003, sein Daemon laeuft nicht.
echo /home/erin/.config/systemd/user/humanitld.service >"$state/fragment-1004"
echo retired >"$state/retire-1004"
# gina: ihr Daemon ist 0.0.6 und kennt die Bitte nicht.
echo unsupported >"$state/retire-1006"
echo none >"$state/session-1006"
run postinst configure 0.0.6
[[ "$rc" -eq 0 ]] || fail "postinst configure exits with $rc: $(cat "$state/err")"

for uid in 1000 1001 1002 1003 1004 1006; do
  has "as $uid:$((uid + 100)) groups=yes HOME=/home/" "every running manager reloads ($uid)"
  has "XDG_RUNTIME_DIR=/run/user/$uid DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/$uid/bus: systemctl --user daemon-reload" \
    "daemon-reload runs as the human with the runtime dir and bus of $uid"
done
lacks "as 1005:" "frank has no running user manager and is not touched"
has "as 1000:1100 groups=yes HOME=/home/alice USER=alice LOGNAME=alice PATH=/usr/bin:/bin" \
  "the environment is empty but for what systemctl --user needs"

# Jeder Aufruf als Mensch: timeout -k 5 des root aussen, dann setsid ohne
# Terminal und mit stdin /dev/null, dann setpriv.
as_user_calls=$(grep -c '^as ' "$state/log" || true)
guarded=$(grep -c '^timeout -k 5 60 setsid setpriv ' "$state/log" || true)
detached=$(grep -c '^setsid stdin=/dev/null stdout=file tty=no next=setpriv$' "$state/log" || true)
[[ "$as_user_calls" -gt 0 && "$guarded" -eq "$as_user_calls" ]] \
  || fail "every call as a human runs under timeout -k 5 outside setsid and setpriv ($guarded of $as_user_calls)"
[[ "$detached" -eq "$as_user_calls" ]] \
  || fail "every call as a human is detached from the terminal, stdin /dev/null, output into a file ($detached of $as_user_calls)"

# alice: Bitte, dann Neustart, in dieser Reihenfolge nach daemon-reload.
reload=$(line_of "/run/user/1000/bus: systemctl --user daemon-reload")
retire=$(line_of "/run/user/1000/bus: humanitl --json daemon retire-if-idle")
restart=$(line_of "/run/user/1000/bus: systemctl --user restart humanitld.service")
if [[ -z "$reload" || -z "$retire" || -z "$restart" || "$reload" -ge "$retire" || "$retire" -ge "$restart" ]]; then
  fail "alice: daemon-reload (${reload:-none}), retire-if-idle (${retire:-none}), restart (${restart:-none}) in this order"
fi
lacks "/run/user/1000/bus: humanitl --json daemon status" "a daemon that knows retire-if-idle is not asked the old way"
for uid in 1001 1002 1003 1004; do
  lacks "/run/user/$uid/bus: systemctl --user restart" "no restart for $uid"
  lacks "/run/user/$uid/bus: systemctl --user try-restart" "no try-restart for $uid"
done
said "an agent session of bob is running" "bob: the note names the running session"
said "could not ask the daemon of carol to retire" "carol: an unreadable answer counts as a running session"
said "systemctl --user restart humanitld" "the notes carry the manual command"
lacks "/run/user/1003/bus: humanitl" "dave's daemon is not running and is not woken by a question"
lacks "/run/user/1004/bus: humanitl" "erin's own unit is not the package's and is not asked"
has "/run/user/1006/bus: humanitl --json daemon status" "gina's old daemon is asked the old way"
has "/run/user/1006/bus: systemctl --user try-restart humanitld.service" "gina's idle old daemon restarts"
has "logger -t humanitl -- restarted the daemon of alice" "the outcome lands in the journal"

# 2. Ein scheiterndes daemon-reload und ein scheiternder Neustart brechen
#    nichts ab, und der naechste Mensch kommt trotzdem dran.
reset_state "1000 alice" "1001 bob"
: >"$state/manager-1000"
: >"$state/manager-1001"
: >"$state/reload-fails-1000"
: >"$state/active-1001"
echo retired >"$state/retire-1001"
: >"$state/restart-fails-1001"
run postinst configure 0.0.6
[[ "$rc" -eq 0 ]] || fail "a failing step must not fail postinst (exit $rc)"
said "daemon-reload failed for alice" "a failed daemon-reload is noted"
said "run as alice: systemctl --user daemon-reload" "a failed daemon-reload names the manual command"
has "/run/user/1001/bus: systemctl --user restart humanitld.service" "bob comes next all the same"
said "restarting the daemon of bob failed" "a failed restart is noted"

# 3. Ein haengendes systemctl haelt postinst nicht auf.
reset_state "1000 alice" "1001 bob"
: >"$state/manager-1000"
: >"$state/manager-1001"
: >"$state/hang-1000"
: >"$state/active-1001"
echo retired >"$state/retire-1001"
started=$SECONDS
STEP_SECONDS=1 run postinst configure 0.0.6
took=$((SECONDS - started))
[[ "$rc" -eq 0 ]] || fail "postinst with a hanging systemctl exits with $rc"
[[ "$took" -lt 20 ]] || fail "postinst with a hanging systemctl took ${took}s; the timeout did not end it"
has "/run/user/1001/bus: systemctl --user restart humanitld.service" "after the hang the next human comes next"
lacks "/run/user/1000/bus: humanitl" "the hanging daemon of alice counts as not active"

# 4. policy-rc.d mit 101: neu geladen wird, neu gestartet nicht.
reset_state "1000 alice"
: >"$state/manager-1000"
: >"$state/active-1000"
echo retired >"$state/retire-1000"
: >"$state/policy-deny"
run postinst configure 0.0.6
[[ "$rc" -eq 0 ]] || fail "postinst with policy-rc.d 101 exits with $rc"
has "/run/user/1000/bus: systemctl --user daemon-reload" "policy-rc.d does not stop the reload"
lacks "retire-if-idle" "policy-rc.d 101 forbids the restart"
said "policy-rc.d forbids restarting services" "the note names policy-rc.d"

# 5. postrm remove laedt nur neu; postrm upgrade tut nichts.
reset_state "1000 alice"
: >"$state/manager-1000"
: >"$state/active-1000"
echo retired >"$state/retire-1000"
run postrm remove
[[ "$rc" -eq 0 ]] || fail "postrm remove exits with $rc"
has "/run/user/1000/bus: systemctl --user daemon-reload" "postrm remove reloads"
lacks "restart" "postrm remove restarts nothing"
lacks "humanitl --json" "postrm remove asks no daemon (the command line is gone)"
reset_state "1000 alice"
: >"$state/manager-1000"
run postrm purge
has "/run/user/1000/bus: systemctl --user daemon-reload" "postrm purge reloads"
reset_state "1000 alice"
: >"$state/manager-1000"
run postrm upgrade 0.0.7
[[ "$rc" -eq 0 ]] || fail "postrm upgrade exits with $rc"
lacks "systemctl" "postrm upgrade leaves the reload to the new postinst"

# 6. Kein systemd, ein chroot, kein loginctl: nichts geschieht.
reset_state "1000 alice"
: >"$state/manager-1000"
SYSTEMD_DIR="$work/no-such-dir" run postinst configure
[[ "$rc" -eq 0 ]] || fail "postinst without systemd exits with $rc"
lacks "loginctl" "without systemd nobody is looked up"
: >"$state/chroot"
run postinst configure
[[ "$rc" -eq 0 ]] || fail "postinst in a chroot exits with $rc"
lacks "loginctl" "in a chroot nobody is looked up"
rm "$state/chroot"
mv "$stubs/loginctl" "$work/loginctl.away"
rc=0
env -i PATH="$stubs" STUB_STATE="$state" STUB_DIR="$stubs" HUMANITL_SYSTEMD_DIR="$systemd_dir" \
  /bin/sh "$scripts/postinst" configure >"$state/out" 2>"$state/err" || rc=$?
mv "$work/loginctl.away" "$stubs/loginctl"
[[ "$rc" -eq 0 ]] || fail "postinst without loginctl exits with $rc: $(cat "$state/err")"
lacks "systemctl" "without loginctl nothing is reloaded"

# 7. Unbekannte Argumente.
reset_state
run postinst bogus
[[ "$rc" -eq 1 ]] || fail "postinst with an unknown argument exits with $rc, not 1"
run postinst abort-upgrade 0.0.7
[[ "$rc" -eq 0 ]] || fail "postinst abort-upgrade exits with $rc"
lacks "systemctl" "abort-upgrade reloads nothing"

# 8. Die Zwischendateien liegen im Verzeichnis des root und nicht im
#    TMPDIR, und nach dem Lauf liegt keine mehr da.
leftover=$(find "$work/tmp" -type f | wc -l)
[[ "$leftover" -eq 0 ]] || fail "postinst left $leftover temporary file(s) behind"
reset_state "1000 alice"
: >"$state/manager-1000"
: >"$state/active-1000"
echo retired >"$state/retire-1000"
run postinst configure 0.0.6
[[ "$rc" -eq 0 ]] || fail "postinst with an unusable TMPDIR exits with $rc"
has "/run/user/1000/bus: systemctl --user restart humanitld.service" "TMPDIR is not where the output goes"
leftover=$(find "$work/tmp" -type f | wc -l)
[[ "$leftover" -eq 0 ]] || fail "postinst left $leftover temporary file(s) behind"

# --- shellcheck --------------------------------------------------------------

if command -v shellcheck >/dev/null 2>&1; then
  shellcheck -s sh "$scripts/postinst" "$scripts/postrm" || fail "shellcheck found problems"
  shellcheck "$here/maintscripts.sh" "$0" || fail "shellcheck found problems in the tooling"
else
  echo "note: shellcheck is not installed; skipped (CI runs it)"
fi

if [[ "$failures" -ne 0 ]]; then
  echo "maintscripts-test: $failures failure(s)" >&2
  exit 1
fi
echo "maintscripts-test: all cases passed"
