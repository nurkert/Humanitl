# shellcheck shell=sh
# Der gemeinsame Teil von `postinst` und `postrm` (HUM-284): die laufenden
# systemd-Nutzerinstanzen finden und in jeder davon, als ihr Mensch,
# `daemon-reload` fahren; nach einem Update ausserdem den Daemon ohne Sitzung
# mit dem neuen Programm neu starten.
#
# Kein eigenstaendiges Skript: `packaging/deb/maintscripts.sh` setzt diese
# Datei an die Stelle der Markenzeile in `postinst.in` und `postrm.in`. So
# steht dieselbe Fassung in beiden Skripten, und `postrm` braucht keine Datei
# des Pakets mehr, wenn es laeuft (dann ist sie schon entfernt).
#
# Warum ueberhaupt: Ein Paket-Update ersetzt `humanitld` und die Units, aber
# die Nutzerinstanz von systemd kennt noch die alte Unit und laesst den alten
# Daemon laufen; `systemctl --user` warnt dann "changed on disk, run
# daemon-reload". Ein Mensch soll danach keinen Befehl tippen muessen.
#
# Die Zusagen:
#
# - **Nie ein Abbruch und nie ein Haenger der Installation.** Jeder Schritt
#   darf scheitern; dann steht eine Zeile mit dem Befehl fuer Hand im Journal
#   (`logger`) und auf stderr, und `dpkg` macht weiter. Jeder Aufruf als
#   Mensch laeuft unter einem `timeout` des root, der ausserhalb von `setpriv`
#   steht: Der Mensch kann ihn weder anhalten noch beenden. Die Ausgabe geht
#   in eine Datei und nie in eine Pipe, die ein Kind im Hintergrund offen
#   halten koennte.
# - **Nie das Terminal des root.** `setsid` trennt jeden Aufruf vom Terminal,
#   stdin ist `/dev/null`, stdout und stderr gehen in die Datei.
# - **Nie ein Neustart mitten in einer Sitzung.** Der Daemon entscheidet das
#   selbst und atomar: `humanitl daemon retire-if-idle` bittet ihn, sich zu
#   verabschieden, und er tut es nur, wenn kein Start einen Anspruch und keine
#   Sandbox ein Handle haelt; ab da bekommt kein Start mehr einen. Erst danach
#   startet `systemctl --user restart` das neue Programm. Ein Daemon vor
#   Vertrag 1.19 (0.0.6) kennt die Bitte nicht; fuer ihn bleibt die Frage
#   ueber `daemon status` mit `try-restart`. Dieser alte Daemon meldet waehrend
#   eines ganzen Starts `stopped`, bis die Sandbox ihr Handle hat; das Fenster
#   ist also der ganze Start, nicht nur die Zeit zwischen Frage und Neustart.
#   Das betrifft genau das Update von 0.0.6.
# - **Nur als der Mensch, dem die Instanz gehoert.** `postinst` laeuft als
#   root, und `systemctl --user` geht dort nicht. `setpriv` wechselt Nutzer und
#   Gruppen ohne PAM und ohne Sitzung, mit einer leeren Umgebung, in der nur
#   steht, was `systemctl --user` und die Kommandozeile brauchen.
# - **Nicht in einem chroot und nicht ohne systemd**, und nicht, wenn
#   `policy-rc.d` den Neustart verbietet (Exit 101).

# Wo das Paket die Kommandozeile ablegt.
HUMANITL_CLI=/usr/lib/humanitl/bin/humanitl
# Die Unit des Pakets; nur sie wird neu gestartet.
HUMANITL_UNIT_PATH=/usr/lib/systemd/user/humanitld.service
# Wo die Ausgabe eines Aufrufs als Mensch zwischenliegt. Nicht `$TMPDIR`: Mit
# `sudo -E` kaeme das Verzeichnis eines Menschen herein. `/run` gehoert root.
HUMANITL_OUT_DIR=${HUMANITL_OUT_DIR:-/run}
# Hoechstdauer eines Aufrufs als Mensch, in Sekunden; danach `SIGTERM`, fuenf
# Sekunden spaeter `SIGKILL`.
HUMANITL_STEP_SECONDS=${HUMANITL_STEP_SECONDS:-60}
# Woran ein laufender systemd zu erkennen ist (sd_booted(3)).
HUMANITL_SYSTEMD_DIR=${HUMANITL_SYSTEMD_DIR:-/run/systemd/system}

# Eine Zeile fuer Mensch und Journal. `logger` kommt aus bsdutils (essential);
# fehlt es trotzdem, bleibt die Zeile auf stderr.
humanitl_note() {
  echo "humanitl: $*" >&2
  logger -t humanitl -- "$*" </dev/null >/dev/null 2>&1 || true
}

# Die Nutzer mit laufender systemd-Nutzerinstanz, je Zeile "uid name".
#
# `loginctl list-users` nennt jeden Menschen mit einer Sitzung oder mit
# `linger`; ob seine Instanz wirklich laeuft, sagt `user@UID.service`. Ohne
# systemd (Container, chroot) bleibt die Liste leer.
humanitl_users() {
  [ -d "$HUMANITL_SYSTEMD_DIR" ] || return 0
  if command -v systemd-detect-virt >/dev/null 2>&1 \
    && systemd-detect-virt --chroot --quiet </dev/null >/dev/null 2>&1; then
    return 0
  fi
  command -v loginctl >/dev/null 2>&1 || return 0
  command -v systemctl >/dev/null 2>&1 || return 0
  timeout -k 5 "$HUMANITL_STEP_SECONDS" loginctl list-users --no-legend </dev/null 2>/dev/null \
    | while read -r uid name _rest; do
      case "$uid" in
        '' | *[!0123456789]*) continue ;;
      esac
      [ -n "$name" ] || continue
      if timeout -k 5 "$HUMANITL_STEP_SECONDS" systemctl is-active --quiet "user@$uid.service" \
        </dev/null >/dev/null 2>&1; then
        printf '%s %s\n' "$uid" "$name"
      fi
    done
}

# Ob `policy-rc.d` den Neustart erlaubt. Ohne die Datei ist alles erlaubt;
# Exit 101 heisst verboten (invoke-rc.d(8)).
humanitl_restart_allowed() {
  command -v policy-rc.d >/dev/null 2>&1 || return 0
  hu_policy=0
  policy-rc.d humanitld restart </dev/null >/dev/null 2>&1 || hu_policy=$?
  [ "$hu_policy" -ne 101 ]
}

# Fuehrt "$@" als der Mensch aus, den `humanitl_refresh_user` gerade bedient
# (`hu_uid`, `hu_gid`, `hu_home`, `hu_name`), mit Frist, ohne Terminal; die
# Ausgabe steht danach in `$hu_out`.
humanitl_as_user() {
  timeout -k 5 "$HUMANITL_STEP_SECONDS" \
    setsid setpriv --reuid="$hu_uid" --regid="$hu_gid" --init-groups \
    env -i HOME="$hu_home" USER="$hu_name" LOGNAME="$hu_name" PATH=/usr/bin:/bin \
    XDG_RUNTIME_DIR="/run/user/$hu_uid" \
    DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$hu_uid/bus" \
    "$@" </dev/null >"$hu_out" 2>&1
}

# `systemctl --user "$@"` als dieser Mensch.
#
# Eine eigene Funktion, damit `systemctl` nie am Anfang einer Zeile steht:
# lintian liest das als Aufruf an den systemd des Systems
# (`maintainer-script-calls-systemctl`), und um den geht es hier nicht.
humanitl_user_systemctl() {
  humanitl_as_user systemctl --user "$@"
}

# Die Ausgabe des letzten Aufrufs als eine kurze Zeile, fuer eine Notiz.
humanitl_said() {
  tr '\n' ' ' <"$hu_out" 2>/dev/null | cut -c1-200
}

# Startet den Daemon nach einer Bitte, die `retired` ergab, mit dem neuen
# Programm.
humanitl_restart_retired() {
  if humanitl_user_systemctl restart humanitld.service; then
    humanitl_note "restarted the daemon of $hu_name with the new version"
  else
    humanitl_note "restarting the daemon of $hu_name failed ($(humanitl_said)); its next connection starts the new version, or run as $hu_name: systemctl --user restart humanitld"
  fi
}

# Der Weg fuer einen Daemon vor Vertrag 1.19, der `retire-if-idle` nicht
# kennt: fragen, dann `try-restart`. Dieser Daemon meldet waehrend eines
# ganzen Starts `stopped`; ein Start, der schon laeuft oder bis zum Neustart
# beginnt, faellt ihm zum Opfer. Das ist das Restfenster genau dieses einen
# Updates.
humanitl_restart_old() {
  humanitl_as_user "$HUMANITL_CLI" --json daemon status || true
  case "$(cat "$hu_out" 2>/dev/null)" in
    *'"agent_session":"none"'*)
      if humanitl_user_systemctl try-restart humanitld.service; then
        humanitl_note "restarted the daemon of $hu_name with the new version"
      else
        humanitl_note "restarting the daemon of $hu_name failed ($(humanitl_said)); run as $hu_name: systemctl --user restart humanitld"
      fi
      ;;
    *'"agent_session":"running"'*)
      humanitl_note "an agent session of $hu_name is running, so its daemon keeps running; run as $hu_name once the session has ended: systemctl --user restart humanitld"
      ;;
    *)
      humanitl_note "could not ask the daemon of $hu_name whether an agent session is running, so it keeps running; run as $hu_name once no session runs: systemctl --user restart humanitld"
      ;;
  esac
}

# Laedt die Units des Menschen mit uid $2 und Namen $3 neu und startet mit
# `restart` als $1 seinen Daemon neu, wenn der laeuft, aus dem Paket kommt
# und keine Sitzung ihn haelt.
humanitl_refresh_user() {
  hu_mode=$1 hu_uid=$2 hu_name=$3
  hu_entry=$(getent passwd "$hu_uid" </dev/null) || {
    humanitl_note "no passwd entry for uid $hu_uid; run as $hu_name: systemctl --user daemon-reload"
    return 0
  }
  hu_gid=$(echo "$hu_entry" | cut -d: -f4)
  hu_home=$(echo "$hu_entry" | cut -d: -f6)
  hu_out=$(mktemp -p "$HUMANITL_OUT_DIR" humanitl-postinst.XXXXXX) || {
    humanitl_note "no temporary file for $hu_name; run as $hu_name: systemctl --user daemon-reload"
    return 0
  }
  # Auch ein Abbruch mitten in den Schritten laesst die Datei nicht liegen.
  trap 'rm -f "$hu_out"' EXIT
  humanitl_refresh_steps
  rm -f "$hu_out"
  trap - EXIT
}

# Die Schritte von `humanitl_refresh_user`, mit `$hu_out` angelegt.
humanitl_refresh_steps() {
  if ! humanitl_user_systemctl daemon-reload; then
    humanitl_note "daemon-reload failed for $hu_name ($(humanitl_said)); run as $hu_name: systemctl --user daemon-reload"
    return 0
  fi
  [ "$hu_mode" = restart ] || return 0
  if ! humanitl_restart_allowed; then
    humanitl_note "policy-rc.d forbids restarting services; the daemon of $hu_name keeps running (run as $hu_name: systemctl --user restart humanitld)"
    return 0
  fi

  # Ein Daemon, der nicht laeuft, braucht nichts: Die naechste Verbindung
  # startet ueber den Socket schon das neue Programm.
  humanitl_user_systemctl is-active --quiet humanitld.service || return 0
  # Eine eigene Unit des Menschen (AppImage, Archiv) verdeckt die des Pakets
  # und startet ein anderes Programm, das dieses Update nicht ersetzt hat.
  humanitl_user_systemctl show --property=FragmentPath --value humanitld.service || return 0
  [ "$(cat "$hu_out")" = "$HUMANITL_UNIT_PATH" ] || return 0

  humanitl_as_user "$HUMANITL_CLI" --json daemon retire-if-idle || true
  case "$(cat "$hu_out" 2>/dev/null)" in
    *'"outcome":"retired"'*) humanitl_restart_retired ;;
    *'"outcome":"busy"'*)
      humanitl_note "an agent session of $hu_name is running, so its daemon keeps running; it ends after the session and the next connection starts the new version"
      ;;
    *'"outcome":"none"'*) ;;
    *'"outcome":"unsupported"'*) humanitl_restart_old ;;
    *)
      humanitl_note "could not ask the daemon of $hu_name to retire ($(humanitl_said)), so it keeps running; run as $hu_name once no session runs: systemctl --user restart humanitld"
      ;;
  esac
}

# Jeder Mensch mit laufender Nutzerinstanz; $1 ist `reload` oder `restart`.
humanitl_refresh_users() {
  humanitl_users | while read -r uid name; do
    humanitl_refresh_user "$1" "$uid" "$name" </dev/null || true
  done
}
