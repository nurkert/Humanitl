#!/usr/bin/env bash
# Prueft ein Paket-Update mit laufendem Daemon auf einem CI-Laeufer (HUM-284).
#
# Aufruf (als gewoehnlicher Nutzer mit sudo, nur in GitHub Actions):
#   check-upgrade.sh <new-deb> <new-version>
#
# Nie auf einem echten Rechner ausfuehren: Das Skript installiert mit sudo
# zwei Fassungen des Pakets, schaltet `linger` fuer den Nutzer ein und startet
# dessen Daemon. Es weigert sich ausserhalb von GitHub Actions.
#
# Der Ablauf ist der Fall des Eigentuemers vom 2026-10-22, ohne Handgriff:
#   1. die letzte veroeffentlichte Fassung (OLD_VERSION, Pruefsumme fest)
#      installieren, die Nutzerinstanz von systemd starten, `humanitl daemon
#      install`, und der Daemon dieser Fassung antwortet;
#   2. das neue Paket darueber installieren;
#   3. ohne einen weiteren Befehl antwortet der Daemon der neuen Fassung,
#      `humanitl doctor` nennt sie, und systemd verlangt kein `daemon-reload`
#      mehr.
#
# Die neue Fassung aus dem CI-Bau heisst 0.0.0 und ist damit kleiner als die
# alte; deshalb `--allow-downgrades`. Fuer dpkg ist es derselbe Weg wie ein
# Update: `postinst configure` der neuen Fassung laeuft.
set -euo pipefail

OLD_VERSION=0.0.6
OLD_SHA256=7866b5ec8a49711016218d76cd55a233d43bb5cabd4a32c7a7726c61220c00c3
OLD_URL="https://github.com/nurkert/Humanitl/releases/download/v$OLD_VERSION/humanitl_${OLD_VERSION}_amd64.deb"

[[ $# -eq 2 ]] || { echo "usage: $0 <new-deb> <new-version>" >&2; exit 2; }
new_deb="$(readlink -f "$1")"
new_version="$2"
[[ "${GITHUB_ACTIONS:-}" == true ]] \
  || { echo "error: this installs packages with sudo and runs only in GitHub Actions" >&2; exit 1; }
[[ "$(id -u)" -ne 0 ]] || { echo "error: run as the ordinary CI user, not as root" >&2; exit 1; }
[[ "$new_version" != "$OLD_VERSION" ]] \
  || { echo "error: the new version must differ from $OLD_VERSION to be told apart" >&2; exit 1; }

uid="$(id -u)"
export XDG_RUNTIME_DIR="/run/user/$uid"
export DBUS_SESSION_BUS_ADDRESS="unix:path=$XDG_RUNTIME_DIR/bus"
export DEBIAN_FRONTEND=noninteractive
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Wartet bis zu $1 Sekunden, bis "${@:2}" gelingt.
wait_for() {
  local seconds="$1"
  shift
  local deadline=$((SECONDS + seconds))
  until "$@" >/dev/null 2>&1; do
    if ((SECONDS >= deadline)); then
      return 1
    fi
    sleep 1
  done
}

# Die Fassung, die der laufende Daemon nennt, oder nichts.
daemon_version() {
  humanitl --json daemon status 2>/dev/null \
    | python3 -c 'import json, sys; print(json.load(sys.stdin).get("daemon_version", ""))' 2>/dev/null \
    || true
}

daemon_is() {
  [[ "$(daemon_version)" == "$1" ]]
}

on_failure() {
  echo "--- systemctl --user status humanitld.service humanitld.socket" >&2
  systemctl --user status --no-pager humanitld.service humanitld.socket >&2 || true
  echo "--- journalctl --user -u humanitld.service (last 80 lines)" >&2
  journalctl --user -u humanitld.service --no-pager -n 80 >&2 || true
}

echo "== 1. humanitl $OLD_VERSION with a running daemon"
curl -fsSL --retry 3 -o "$work/old.deb" "$OLD_URL"
echo "$OLD_SHA256  $work/old.deb" | sha256sum -c -
sudo apt-get install -y --no-install-recommends "$work/old.deb"
sudo loginctl enable-linger "$USER"
wait_for 60 test -S "$XDG_RUNTIME_DIR/bus" \
  || { echo "error: the user manager of $USER did not come up" >&2; exit 1; }
wait_for 30 systemctl --user show-environment \
  || { echo "error: systemctl --user does not answer" >&2; exit 1; }
humanitl daemon install
if ! wait_for 60 daemon_is "$OLD_VERSION"; then
  echo "error: the daemon of $OLD_VERSION did not answer (it says '$(daemon_version)')" >&2
  on_failure
  exit 1
fi
systemctl --user is-active --quiet humanitld.service \
  || { echo "error: humanitld.service is not active before the upgrade" >&2; on_failure; exit 1; }
echo "the daemon of $OLD_VERSION runs: $(systemctl --user show -p MainPID --value humanitld.service)"

echo "== 2. install the new package over it"
sudo apt-get install -y --no-install-recommends --allow-downgrades "$new_deb" 2>&1 | tee "$work/apt.log"
grep -q "restarted the daemon of $USER" "$work/apt.log" \
  || { echo "error: postinst did not report the restart of the daemon of $USER" >&2; on_failure; exit 1; }

echo "== 3. the new daemon runs, without a manual step"
if ! wait_for 30 daemon_is "$new_version"; then
  echo "error: after the upgrade the daemon says '$(daemon_version)', not $new_version" >&2
  on_failure
  exit 1
fi
reload="$(systemctl --user show -p NeedDaemonReload --value humanitld.service)"
[[ "$reload" == no ]] \
  || { echo "error: systemd still wants daemon-reload for humanitld.service (NeedDaemonReload=$reload)" >&2; exit 1; }
doctor="$(humanitl doctor 2>&1 || true)"
echo "$doctor"
grep -q "humanitld $new_version," <<<"$doctor" \
  || { echo "error: humanitl doctor does not report humanitld $new_version" >&2; on_failure; exit 1; }
echo "== check-upgrade: $OLD_VERSION to $new_version left the new daemon running, no manual step"
