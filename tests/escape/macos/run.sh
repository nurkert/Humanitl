#!/usr/bin/env bash
# Escape-Tests für das Seatbelt-Backend unter macOS (HUM-323).
#
# Startet einen Daemon in einem eigenen, leeren HOME und TMPDIR, fährt
# `humanitl sandbox check` und versucht dann in einer Sandbox genau das, was
# unter macOS nie gelingen darf: am Proxy vorbei ins Netz, Namen auflösen,
# fremde Sockets erreichen, das Heimverzeichnis lesen, Masken lesen oder
# beschreiben. Die Aussagen stehen in `docs/SECURITY.md` Abschnitt 6.
#
#   tests/escape/macos/run.sh [<bin-dir>]
#
# <bin-dir> enthält humanitl, humanitld und humanitl-shim; Vorgabe ist
# daemon/target/debug nach einem `cargo build`.
#
# Exit 0: alles gehalten. 1: eine Probe kam durch. 2: keine Sandbox.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
bin="${1:-$root/daemon/target/debug}"
# Absolut, denn die Proben laufen aus dem Projektordner.
bin="$(cd "$bin" 2>/dev/null && pwd -P || echo "$bin")"
for tool in humanitl humanitld humanitl-shim; do
  if [[ ! -x "$bin/$tool" ]]; then
    echo "missing $bin/$tool; build with: cd daemon && cargo build -p humanitl -p humanitld -p humanitl-shim" >&2
    exit 2
  fi
done
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "this suite runs on macOS only" >&2
  exit 2
fi

# Kurz genug für sun_path (104 Zeichen) und außerhalb des echten HOME.
work="$(mktemp -d "${HUMANITL_ESC_TMP:-/tmp}/hum-esc.XXXXXX")"
work="$(cd "$work" && pwd -P)"
daemon_pid=""
cleanup() {
  if [[ -n "$daemon_pid" ]]; then
    kill "$daemon_pid" 2>/dev/null || true
    wait "$daemon_pid" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

export HOME="$work/home"
export TMPDIR="$work/t/"
mkdir -p "$HOME/project/.git/hooks" "$HOME/gitproject" "$TMPDIR"
chmod 700 "$TMPDIR"
project="$HOME/project"
echo "secret" > "$project/.env"
echo "readme" > "$project/readme"
echo "[core]" > "$project/.git/config"
echo "machine example.com password hunter2" > "$HOME/.netrc"
# Ein echtes Repository, denn `$project/.git` trägt nur die Maske.
gitproject="$HOME/gitproject"
echo "readme" > "$gitproject/readme"
git -C "$gitproject" init -q

# Mit einem Endpunkt legt der Adapter seine Dateien an (Probe `adapter_files`).
"$bin/humanitl" config set llm.endpoint http://127.0.0.1:11434/v1 >/dev/null 2>&1
"$bin/humanitld" > "$work/daemon.log" 2>&1 &
daemon_pid=$!
for _ in $(seq 1 50); do
  "$bin/humanitl" daemon status >/dev/null 2>&1 && break
  sleep 0.2
done

failed=0
pass() { printf 'PASS  %s\n' "$1"; }
fail() { printf 'FAIL  %s: %s\n' "$1" "$2"; failed=1; }

if check="$(cd "$project" && "$bin/humanitl" sandbox check 2>&1)"; then
  if [[ "$(grep -c '^✓' <<<"$check")" == 3 ]]; then
    pass "sandbox check: three guarantees"
  else
    fail "sandbox check" "$check"
  fi
else
  echo "$check" >&2
  echo "the sandbox did not start" >&2
  exit 2
fi

# Jede Probe druckt genau eine Zeile `<name> <ok|LEAK> <beleg>`.
probes='
probe() { name="$1"; shift; if "$@" >/dev/null 2>&1; then echo "$name LEAK"; else echo "$name ok"; fi; }
probe direct_tcp       curl -sS --max-time 3 --noproxy "*" http://1.1.1.1/
probe direct_tls       curl -sS --max-time 3 --noproxy "*" https://example.com/
probe dns              python3 -c "import socket; socket.getaddrinfo(\"example.com\", 80)"
probe udp              python3 -c "import socket; socket.socket(2,2).sendto(b\"x\", (\"8.8.8.8\", 53))"
probe ipv6             python3 -c "import socket; socket.create_connection((\"2606:4700:4700::1111\", 443), 3)"
probe raw_socket       python3 -c "import socket; socket.socket(2, 3, 1)"
probe route_socket     python3 -c "import socket; socket.socket(17, 3, 0)"
probe syslog_socket    python3 -c "import socket; socket.socket(1).connect(\"/var/run/syslog\")"
probe mdns_socket      python3 -c "import socket; socket.socket(1).connect(\"/var/run/mDNSResponder\")"
probe other_loopback   python3 -c "import socket; socket.create_connection((\"127.0.0.1\", 22), 3)"
probe read_home        cat "$REAL_HOME/.netrc"
probe list_home        ls "$REAL_HOME"
probe read_users       ls /Users
probe read_env_mask    cat .env
probe write_git_hooks  sh -c "echo x > .git/hooks/pre-commit"
probe write_outside    sh -c "echo x > /private/tmp/hum-esc-leak"
probe write_tmp        python3 -c "open(\"/\" + \"tmp/hum-esc-leak\", \"w\")"
probe read_git_config  cat .git/config
probe read_var_tmp     ls /private/var/tmp
probe nested_sandbox   sandbox-exec -p "(version 1)(allow default)" /usr/bin/true
probe write_library    sh -c "echo x > /Library/hum-esc-leak"
probe write_profile    sh -c "echo \"(version 1)(allow default)\" > \"\$HOME/../sandbox.sb\""
probe remove_profile   rm -f "$HOME/../sandbox.sb"
probe write_etc        sh -c "echo x > \"\$HOME/../etc/humanitl/ca.crt\""
probe rename_git       sh -c "mv .git .gitx"
probe other_terminal   sh -c "for t in /dev/ttys*; do [ -c \"\$t\" ] && exec 3<>\"\$t\" && exit 0; done; exit 1"
'
expected_ok='write_project read_project own_pty write_stdout write_fd write_tmpdir'
out="$(cd "$project" && "$bin/humanitl" sandbox run --work "$project" -- /bin/sh -c "
REAL_HOME='$HOME'
$probes
echo x > new.txt && echo write_project works
cat readme >/dev/null && echo read_project works
sh -c 'echo x > /dev/stdout' >/dev/null && echo write_stdout works
sh -c 'echo x > /dev/fd/1' >/dev/null && echo write_fd works
python3 -c 'import os; m, s = os.openpty(); os.write(s, b\"x\"); print(\"own_pty works\")'
echo x > \"\$TMPDIR/probe\" && echo write_tmpdir works
" 2>&1 || true)"
while read -r name verdict _; do
  case "$verdict" in
    ok) pass "$name refused" ;;
    LEAK) fail "$name" "got through" ;;
    works) pass "$name" ;;
  esac
done < <(grep -E ' (ok|LEAK|works)$' <<<"$out")
for name in direct_tcp direct_tls dns udp ipv6 raw_socket route_socket syslog_socket mdns_socket \
  other_loopback read_home list_home read_users read_env_mask write_git_hooks write_outside \
  write_library write_profile remove_profile write_etc rename_git other_terminal write_tmp \
  read_git_config read_var_tmp nested_sandbox $expected_ok; do
  grep -qE "^$name (ok|works)$" <<<"$out" || { grep -qE "^$name LEAK$" <<<"$out" || fail "$name" "no answer: $out"; }
done
[[ -e /private/tmp/hum-esc-leak ]] && fail "write_outside" "file exists" && rm -f /private/tmp/hum-esc-leak

# Eine Probe, deren Zeile fehlt, ist nicht bestanden: Die Schleife oben
# schweigt zu einem `works`, das nie gedruckt wurde.
works_in() {
  local name="$1" dir="$2" script="$3" flags="${4:-}" start="${5:-sandbox run}"
  # shellcheck disable=SC2086
  if (cd "$dir" && "$bin/humanitl" $flags $start --work "$dir" -- /bin/sh -c "$script" 2>&1) |
    grep -q "^$name works"; then
    pass "$name"
  else
    fail "$name" "did not work"
  fi
}

# Git arbeitet in einem Repository, das die Sandbox selbst anlegt. In einem
# echten Repository bleibt `.git/config` maskiert: Unter macOS antwortet die
# Maske mit EPERM, unter Linux ist sie eine leere Datei; das `git` der Sitzung
# kommt ohne sie aus (`docs/SECURITY.md` Abschnitt 12).
works_in git_use "$gitproject" \
  'mkdir inner && cd inner && git init -q && echo x > f && git add f && git -c user.name=a -c user.email=a@b commit -qm x && git log --oneline | grep -q x && echo git_use works'
works_in git_config_stays_masked "$gitproject" \
  'cat .git/config >/dev/null 2>&1 || echo git_config_stays_masked works'

# Die Dateien des Adapters kommen an, der Text mit seinem `/tmp` unverändert.
works_in adapter_files "$project" \
  'grep -q "[/]tmp\." "${XDG_CONFIG_HOME:-$HOME/.config}/opencode/AGENTS.md" && test -r "${XDG_CONFIG_HOME:-$HOME/.config}/opencode/opencode.json" && echo adapter_files works' \
  '' run

# Ein nur lesbares Projekt: lesen geht, schreiben endet am Profil.
works_in ro_project_reads "$project" 'cat readme >/dev/null && echo ro_project_reads works' \
  '--work-mode ro'
works_in ro_project_refuses_writes "$project" \
  'echo x > ro-leak 2>/dev/null || echo ro_project_refuses_writes works' '--work-mode ro'
[[ -e "$project/ro-leak" ]] && fail "ro_project_refuses_writes" "file exists"

# Ohne PID-Namensraum endet ein Hintergrundprozess des Agenten nicht von
# selbst mit ihm; der Shim beendet die Sitzung des Kommandos.
(cd "$project" && "$bin/humanitl" sandbox run --work "$project" -- \
  /bin/sh -c 'sleep 7771 & exit 0' >/dev/null 2>&1 || true)
sleep 1
if pgrep -f "sleep 7771" >/dev/null; then
  fail "background_process" "outlived the sandbox"
  pkill -f "sleep 7771" || true
else
  pass "a background process ends with the sandbox"
fi

# Durch den Proxy kommt eine Anfrage an und wird gehalten, nicht durchgelassen.
(cd "$project" && "$bin/humanitl" sandbox run --work "$project" -- \
  /bin/sh -c 'curl -sS --max-time 3 https://example.com/ -o /dev/null' >/dev/null 2>&1 || true)
if "$bin/humanitl" flows list 2>/dev/null | grep -q 'example.com'; then
  pass "the proxied request reached the daemon"
else
  fail "proxy" "no flow for example.com; daemon log: $(tail -5 "$work/daemon.log")"
fi

exit "$failed"
