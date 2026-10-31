---
source: INSTALL.md
source-sha256: b99d75439862eb0795f5c419036fc66f792ccabced590d76026a8b841c044a47
lang: de
---
# Installation: Notizen für Maintainer

Applies to: Humanitl 0.1

**Um Humanitl zu installieren, lies [die Installationsanleitung](guides/install.md).** Sie
behandelt die Voraussetzungen, das `.deb`, das `.rpm`, das AUR-Paket, das AppImage, das
Archiv, das macOS-Disk-Image, das Aktivieren des User-Dienstes, die Prüfung der Installation
und die Deinstallation. Wie die macOS-App und ihr Disk-Image gebaut werden, steht in
`packaging/macos/` (HUM-326).

Diese Seite bewahrt, was die Anleitung weglässt: das APT-Repository, den Wechsel zwischen
Installationsformen, warum die systemd-Unit so gehärtet ist, wie sie ist, das
Laufzeitverzeichnis ohne logind, das Bauen der Pakete und die manuelle Testmatrix.
Ausgelieferter Code verlinkt auf drei Abschnitte dieser Seite (`#hardening`,
`#xdg_runtime_dir-ohne-logind` und `#runtime-directory-on-macos`); alle drei Anker bleiben.

## APT-Repository

Jedes Release landet auch im APT-Repository `apt.nurkert.de`, sodass spätere Versionen mit
`apt upgrade` ankommen:

```sh
curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh
humanitl daemon install
humanitl daemon status
```

Das Skript legt den Signaturschlüssel des Repositorys unter
`/usr/share/keyrings/nurkert-archive-keyring.gpg` ab, die Quelle unter
`/etc/apt/sources.list.d/nurkert.list`, und installiert das Paket. Wer einer Pipe in eine
Root-Shell nicht traut, liest das Skript zuerst oder nimmt das `.deb` von der Release-Seite,
wie die Anleitung es beschreibt.

## Das Paket im Detail

Menü und Icon-Cache aktualisieren die Trigger von `desktop-file-utils` und
`hicolor-icon-theme`. Das Paket hat zwei Maintainer-Skripte, beide zum User-Dienst
(HUM-284):

- `postinst` findet jeden Nutzer mit laufendem systemd-User-Manager
  (`loginctl list-users`, dann `user@UID.service`; nicht in einem chroot, nicht ohne
  systemd) und führt als dieser Nutzer über `setpriv` `systemctl --user
  daemon-reload` aus.
  Ist `humanitld.service` dieses Nutzers aktiv und stammt aus dem Paket, führt es
  `humanitl daemon retire-if-idle` aus: Der Daemon entscheidet selbst, unter der Sperre, mit
  der ein Start seinen Anspruch anmeldet, dass ihn keine Agenten-Session hält, und von da an
  kann keine Session mehr starten. Erst nach `retired` startet
  `systemctl --user restart humanitld.service` die neue Version. Eine laufende Session behält
  ihren Daemon; ein Daemon ab dieser Version beendet sich von selbst, sobald die Session
  vorbei ist, und die nächste Verbindung startet das neue Programm über den Socket. Bis dahin
  zeigt die App den Versionshinweis mit einem Knopf „Daemon neu starten". Der Daemon von 0.0.6
  kennt die Anfrage nicht; für ihn fragt `postinst` `humanitl --json daemon status` und führt
  `try-restart` nur bei `agent_session` `none` aus. Dieser alte Daemon meldet während eines
  ganzen Starts `stopped`, also kann ein Start, der vor dem Neustart läuft oder beginnt,
  abgeschnitten werden; nur dieses eine Upgrade hat dieses Zeitfenster. Jeder Aufruf als der
  Nutzer läuft unter einem `timeout` auf der Root-Seite, vom Terminal gelöst, und eine
  `policy-rc.d`, die 101 liefert, verbietet den Neustart.
- `postrm` führt dasselbe `daemon-reload` nach `remove` und `purge` aus.

Keines von beiden lässt die Installation je scheitern. Was nicht geklappt hat, steht im
Journal (`journalctl -t humanitl`) und in der Ausgabe von apt, mit dem Befehl, den man von
Hand ausführen muss, meist `systemctl --user restart humanitld`.

`humanitl daemon install` aktiviert nur den Socket, nicht den Dienst (HUM-164). Wer
`humanitl daemon install` vor dieser Änderung ausgeführt hat, hat auch den Dienst aktiviert;
ein neuer Aufruf entfernt diesen Link nicht, und der Dienst startet weiter mit der Sitzung.
Das schadet nicht. Damit er erst beim ersten Client startet, führst du einmal
`systemctl --user disable humanitld.service` aus; der Socket bleibt aktiviert.

## Wechsel zwischen Installationsformen

Wer vorher aus dem Archiv oder dem AppImage installiert hat, hat noch die Unit, die
`humanitl daemon install` unter `~/.config/systemd/user/` geschrieben hat. Sie verdeckt die
Unit des Pakets. `humanitl daemon install` aus dem Paket erkennt diese Unit an ihrer ersten
Zeile, kündigt sie an, schiebt sie und ihren Aktivierungslink als `humanitld.service.bak`
beiseite (oder `.bak.1`, `.bak.2` und so weiter, wenn der Name vergeben ist) und startet den
Dienst neu, damit die Unit des Pakets läuft. Unter `--no-start` oder ohne `systemctl` bleibt
sie, wo sie ist, und die Ausgabe sagt, dass sie die Unit des Pakets weiter verdeckt.

Das Beiseiteschieben nutzt `renameat2` mit `RENAME_NOREPLACE`. Liegt `~/.config` auf NFS,
das diesen Aufruf mit `EINVAL` abweist, bricht der Befehl mit `DAEMON_006` ab und lässt alles,
wie es war. Eine Datei ohne die erste Zeile von `daemon install` wird nie angefasst; der
Befehl bricht mit `DAEMON_005` ab. Diese Datei gehört jemand anderem, der sie entfernt oder
die Änderungen nach `systemctl --user edit` überträgt.

## Härtung

`packaging/systemd/humanitld.service` ist so weit gehärtet, wie die Sandbox es zulässt. Jede
Zeile wurde am 2026-09-18 auf Debian 14 mit systemd 262 und bubblewrap 0.12.0 gemessen, mit
`systemd-run --user` und denselben Eigenschaften: Die Escape-Tests (`tests/escape/run.sh`)
bestehen darunter mit 124 von 124 Fällen, einschließlich der drei Sandbox-Garantien und des
Terminals des Agenten; ein beschreibbares Projekt unter `$HOME` ist in der Sandbox
beschreibbar; node, python3 und java laufen darunter. Das ist wichtig, weil **jedes Kind des
Daemons die Einschränkungen der Unit erbt**: das Sandbox-Backend, der Shim und der Agent. Was
hier steht, gilt auch für den Agenten.

`systemd-analyze --user security --offline=true` auf die Unit mit dem Pfad des Pakets:

Overall exposure level: 3.7 (systemd 262, Debian 14, 2026-09-18; Grenze aus HUM-053: 4.0)

Der Test `the_exposure_stays_at_or_below_the_documented_value` in
`daemon/bin/humanitl/src/cmd/unit.rs` liest den Wert oben, misst die Unit bei jedem Lauf, bei
dem `systemd-analyze` es kann, erneut und hält beide bei 4,0 oder darunter.

### Was gesetzt ist

| Zeile | Grund |
|---|---|
| `Type=notify` | Der Daemon sendet `READY=1`, sobald Token und Socket bereitstehen. |
| `NoNewPrivileges=yes` | bubblewrap läuft ohne setuid über User-Namespaces und braucht keine neuen Rechte. |
| `PrivateTmp=yes` | Das `/tmp` des Daemons ist sein eigenes. Ein Projekt unter `/tmp` ist für ihn deshalb unsichtbar. |
| `ProtectSystem=strict` | In einer User-Unit ist danach alles schreibgeschützt außer `$HOME`, dem Laufzeitverzeichnis und den API-Dateisystemen (gemessen, nicht nachgelesen). |
| `ReadWritePaths=-%h/.local/share/humanitl -%h/.config/humanitl -%t/humanitl` | Die drei Verzeichnisse des Daemons, jeweils mit `-`, weil systemd eine Unit mit fehlendem Pfad nicht startet. |
| `PrivateIPC=yes`, `PrivateMounts=yes`, `PrivateUsers=yes` | Eigene IPC- und Mount-Namespaces, nur der eigene Nutzer sichtbar. |
| `ProtectClock=yes`, `ProtectControlGroups=yes`, `ProtectKernelModules=yes` | Uhr, cgroups und Kernel-Module gehen den Daemon nichts an. |
| `ProtectProc=invisible` | Prozesse anderer Nutzer sind unsichtbar. |
| `KeyringMode=private`, `LockPersonality=yes`, `RestrictRealtime=yes` | Übliche Härtung ohne Wirkung auf die Sandbox. |
| `RestrictFileSystems=~@privileged-api` | Keine Dateisysteme wie `bpf` oder `debugfs`. |
| `CapabilityBoundingSet=CAP_SYS_ADMIN` | Ein User-Dienst hat keine Capabilities; die Grenze wirkt auf Programme mit Datei-Capabilities. Sie darf nicht leer sein (siehe unten). |
| `SystemCallFilter=@system-service @mount @sandbox sethostname`, `SystemCallErrorNumber=EPERM` | `@mount`, weil bubblewrap einhängt und `pivot_root` aufruft; `@sandbox`, weil der Shim mit `seccomp(2)` seinen eigenen seccomp-Filter setzt; `sethostname`, weil bubblewrap den UTS-Namespace der Sandbox `sandbox` nennt (ohne das „Can't set hostname to sandbox"). Der Hostname des Rechners ändert sich nicht. |
| `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK` | bubblewrap braucht `AF_NETLINK` für `lo` im Namespace, der Proxy braucht IPv4 und IPv6, um das Ziel zu erreichen. |

### Was mit Absicht fehlt

| Zeile | Gemessene Folge |
|---|---|
| `RestrictNamespaces` | bubblewrap: „No permissions to create a new namespace". Die Sandbox braucht user, mnt, pid, net, ipc und uts. |
| `ProtectKernelTunables=yes` | bubblewrap: „Can't mount proc on /proc: Operation not permitted". Ein verdecktes `/proc/sys` verbietet ein neues `/proc`. |
| `ProtectKernelLogs=yes` | Derselbe Fehler, wegen des verdeckten `/proc/kmsg`. |
| `ProtectHostname=yes` | Allein harmlos; zusammen mit den anderen Zeilen derselbe Fehler, wenn `/proc` eingehängt wird. |
| `RestrictSUIDSGID=yes` | bubblewrap: „Can't open source /usr: Function not implemented". |
| `MemoryDenyWriteExecute=yes` | Vom Agenten geerbt: node (V8) bricht beim Start mit „Check failed: 12 == (*__errno_location ())" ab, ebenso jede andere JIT-Laufzeit. |
| `CapabilityBoundingSet=` (leer) | bubblewrap startet nicht. |
| `SystemCallArchitectures=native` | Der Shim prüft seinen eigenen Filter mit einem x32-Aufruf von `socket` und stirbt an `SIGSYS`; die Sandbox startet nicht (`SANDBOX_016`, „families not reported"). |
| `PrivateDevices=yes` | Ohne `/dev/ptmx` öffnet der Daemon kein Terminal für den Agenten (`TERM_002`, „cannot open a pseudo terminal"). |
| `UMask=0077` | Vom Agenten geerbt: Jede Datei, die er im Projekt anlegt, bekäme andere Rechte als ohne Humanitl. |
| `ProtectHome=read-only` | Ein Projekt unter `$HOME` wäre in der Sandbox schreibgeschützt; `sandbox.work_mode = rw` ist die Vorgabe. |
| `ProcSubset=pid` | `humanitl doctor` liest `/proc/sys/kernel/*` und `/proc/modules`; diese Erkenntnisse gingen verloren. |
| `PrivateNetwork`, `IPAddressDeny` | Der Proxy muss das Ziel erreichen, das ein Mensch freigibt. |

### Projekte außerhalb von `$HOME`

Unter `ProtectSystem=strict` ist ein Projekt außerhalb des Home-Verzeichnisses, zum Beispiel
unter `/srv` oder auf einer eigenen Platte, für den Daemon und damit in der Sandbox
schreibgeschützt. Ein Drop-in für die Unit macht es beschreibbar:

```sh
systemctl --user edit humanitld.service
# [Service]
# ReadWritePaths=/srv/demo-project
systemctl --user restart humanitld.service
```

Ein Projekt unter `/tmp` ist für den Daemon wegen `PrivateTmp` unsichtbar.

<a id="xdg_runtime_dir-ohne-logind"></a>

## XDG_RUNTIME_DIR ohne logind

Socket und Token des Daemons liegen im Laufzeitverzeichnis `$XDG_RUNTIME_DIR/humanitl`. Ist
`XDG_RUNTIME_DIR` nicht gesetzt und gibt es kein `/run/user/<uid>`, zum Beispiel ohne logind,
weichen Daemon, Kommandozeile und App auf `$TMPDIR/humanitl-<uid>` aus. Jedes andere Konto
kann diesen Namen zuerst anlegen. Humanitl weist ein solches fremdes Verzeichnis zurück
(`DAEMON_001`, `DAEMON_004`) und startet nicht, solange es da ist
([SECURITY.md](SECURITY.md)).

Die Abhilfe ist ein eigenes Laufzeitverzeichnis unter deinem Home-Verzeichnis. Daemon,
Kommandozeile und App müssen dieselbe Variable sehen, also gehört sie in die Anmeldung und
nicht vor einen einzelnen Befehl. Lege das Verzeichnis einmal an:

```sh
mkdir -p "$HOME/.cache/humanitl-run"
chmod 700 "$HOME/.cache/humanitl-run"
```

Füge dann die folgende Zeile in die Datei ein, die deine Anmeldung liest. Sie setzt die
Variable nur, wenn sie fehlt und `/run/user/<uid>` nicht existiert, sodass eine spätere
Sitzung mit logind ihre eigenen Sockets für Wayland, PipeWire und D-Bus behält.

```sh
[ -n "$XDG_RUNTIME_DIR" ] || [ -d "/run/user/$(id -u)" ] || export XDG_RUNTIME_DIR="$HOME/.cache/humanitl-run"
```

Welche Datei das ist, hängt von der Shell ab:

- `bash` liest bei der Anmeldung nur die erste vorhandene Datei von `~/.bash_profile`,
  `~/.bash_login` und `~/.profile`. Die Zeile gehört in genau diese Datei. Lege `~/.profile`
  nur an, wenn weder `~/.bash_profile` noch `~/.bash_login` existiert.
- `zsh` liest `~/.zprofile`, nicht `~/.profile`. Die Zeile gehört in `~/.zprofile`; die Datei
  darf dafür angelegt werden.
- Display-Manager ohne systemd-Sitzung lesen meist `~/.profile`. Wer die App von dort
  startet, fügt die Zeile auch dort ein.

Die Zeile wirkt ab der nächsten Anmeldung. Von da an finden Daemon, Kommandozeile und App
dasselbe Verzeichnis. Dass die Clients es auch ohne diese Zeile finden, ist HUM-222.

## Laufzeitverzeichnis unter macOS

Unter macOS liegen Socket und Token für Daemon, Kommandozeile und App in
`~/Library/Application Support/Humanitl/run`. `XDG_RUNTIME_DIR` gehört nicht zu einer macOS-Sitzung,
und `/run/user` gibt es nicht.

Nur wenn dieser Pfad für einen Unix-Socket zu lang ist (104 Bytes, dazu braucht es einen sehr langen
Kontonamen), nehmen alle drei stattdessen `$TMPDIR/humanitl-<uid>`; dort löscht macOS Dateien, die
drei Tage niemand angefasst hat. Lehnt ein Client das Verzeichnis dort ab, prüfe es mit `ls -ld`: Es
muss deinem Konto gehören und den Modus `0700` haben. Den Modus korrigierst du mit `chmod 700`. Ein
Verzeichnis eines anderen Kontos kann nur ein Administrator entfernen. Starte danach den Daemon neu,
damit er Socket und Token neu schreibt:

```sh
/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon restart
```

## macOS auf einen Blick

Die Kommandozeile liegt in der App und nicht im `PATH`. Die Tabelle nennt den vollen Pfad; die
Anleitung zeigt, wie du sie [verlinkst](guides/install.md#install-on-macos).

| Aufgabe | Unter macOS |
|---|---|
| Installieren | `Humanitl.app` aus dem Disk-Image in Programme ziehen, den ersten Start erlauben, dann „Installieren und starten" im Einrichtungsschirm oder `/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon install`. Das schreibt den launchd-Agenten `~/Library/LaunchAgents/dev.humanitl.humanitld.plist`. |
| Aktualisieren | `Humanitl.app` in Programme durch die neue Fassung ersetzen, dann `/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon restart`. Es gibt keinen Paketmanager und kein `apt upgrade`; der Agent zeigt weiter auf denselben Pfad. |
| Prüfen | `/Applications/Humanitl.app/Contents/MacOS/bin/humanitl doctor`. Die [Zeilen unter macOS](cli.md#the-lines-on-macos) sagen, was dort gemessen wird. |
| Protokoll | `/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon logs`, das `~/Library/Logs/Humanitl/humanitld.log` liest. |
| Deinstallieren | `/Applications/Humanitl.app/Contents/MacOS/bin/humanitl daemon uninstall`, dann die App löschen. |

## Pakete bauen

```sh
make package PACKAGE_VERSION=0.0.N
```

Das Target baut `humanitld`, `humanitl` und den statischen Shim
(`packaging/release/build-binaries.sh`) nach `app/linux/bundle-extra/`, dann das
Flutter-Bundle (`app/linux/CMakeLists.txt` legt die drei nach `bin/`), dann
`dist/humanitl_<version>_amd64.deb` (`packaging/deb/build-deb.sh`) und
`dist/Humanitl-<version>-x86_64.AppImage` (`packaging/appimage/build-appimage.sh`) und prüft
das AppImage (`packaging/appimage/check-appimage.sh`). Es braucht `dpkg-dev`, `patchelf`,
`curl` und für den statischen Shim `musl-tools`. Das Skript lädt `appimagetool` und die
AppImage-Laufzeit in festgelegten Versionen herunter und prüft ihre SHA-256-Summen, bevor es
sie ausführt.

`Depends` des `.deb` wird abgeleitet, nicht von Hand geschrieben: `dpkg-shlibdeps` liest die
Bibliotheken der gebauten Programme, und `build-deb.sh` ergänzt
`bubblewrap (>= MIN_BWRAP_VERSION)` aus `daemon/crates/sandbox/src/bwrap.rs`, `libegl1` und
`libgles2`. Die Pakete werden auf Ubuntu 24.04 gebaut, die abgeleiteten
Bibliotheksversionen sind also die dieses Systems.

Die PNG-Icons unter `packaging/deb/icons/` werden mit Inkscape 1.x aus
`packaging/deb/humanitl.svg` gerendert. Wer das SVG ändert, rendert sie neu und committet die
drei Bilder im selben Commit:

```sh
packaging/deb/icons/render.sh
```

Herkunft und Lizenz des Icons (Font Awesome Free, CC BY 4.0) stehen in `THIRD_PARTY.md` und
im Paket in `/usr/share/doc/humanitl/copyright`.

Das `.deb` wird nie auf einem Arbeitsrechner installiert, sondern in einem Wegwerf-Container
geprüft, mit `packaging/deb/check-install.sh` (Installation, Bibliotheken, Start der App unter
Xvfb, Units, Icons, `daemon install --print`, Entfernen mit `--purge`) und
`packaging/deb/lintian.sh` (keine Fehler, keine Warnungen; jede Ausnahme steht mit ihrem
Grund in `packaging/deb/lintian-overrides`).

## Manuelle Testmatrix

Was kein Container zeigt, prüft ein Mensch auf echter Hardware, bei jedem Pre-Release und vor
0.1.0. Ergebnis und Datum kommen in die Tabelle; eine leere Zelle heißt: noch nicht geprüft.

| Prüfung | Debian 13 GNOME Wayland (Intel) | Debian 13 KDE Wayland (NVIDIA proprietär) | Ubuntu 24.04 GNOME X11 |
|---|---|---|---|
| `sudo apt install ./humanitl_…deb` auf einer frischen VM, dann `humanitl daemon install` und `humanitl daemon status` mit Exit 0 | | | |
| App startet, Einrichten zeigt den Daemon grün | | | |
| Tray-Icon (GNOME braucht die Erweiterung „AppIndicator and KStatusNotifierItem Support"; Einrichten weist darauf hin) | | | |
| Fenster scharf und vollständig bei 200 % Skalierung | | | |
| Benachrichtigung bei einer angehaltenen Anfrage | | | |
| Sandbox-Start mit einem beschreibbaren Projekt unter `$HOME` unter der Unit | | | |
| Impeller rendert (Fenster nicht schwarz, `humanitl doctor` ohne `DOCTOR_010`) | | | |
| AppImage startet ohne mitgeliefertes GTK | | | |
| [Installationsanleitung](guides/install.md) und [erste Session](guides/first-session.md) Schritt für Schritt auf einer frischen Debian-13-VM nur mit dem `.deb` befolgt, bis zur ersten angehaltenen Anfrage, in unter 15 Minuten | | | |

Scheitert Impeller an einem Treiber (schwarzes Fenster, Absturz beim Start), kommt der Befund
mit Treiber und Version in diese Tabelle. Der Release-Build hat noch keinen Rückfall auf einen
anderen Renderer: Schalter der Flutter-Engine wie `--no-enable-impeller` reicht der
Linux-Runner an den Dart-Code weiter, nicht an die Engine, und die Umgebungsvariablen der
Engine gelten laut ihrem Quelltext nur außerhalb von Release-Builds. Beides ist hier nicht
gemessen (HUM-168).
