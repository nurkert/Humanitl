# Installation

Humanitl kommt als `.deb` für Debian und Ubuntu (amd64), als AppImage und als Archiv
(`humanitl-<version>-linux-x86_64.tar.gz`). Alle drei enthalten dieselben Programme: die
Anwendung, die Kommandozeile `humanitl`, den Daemon `humanitld` und den statischen Shim
`humanitl-shim`, der in der Sandbox läuft. Flatpak, Snap, RPM und arm64 kommen nach dem MVP.

Der Daemon läuft immer als **Benutzerdienst** von systemd, nie als System-Dienst und nie mit
`sudo`: Er greift auf Laufzeitverzeichnis, Konfiguration und Aufzeichnung genau eines Menschen zu
(`docs/SECURITY.md`, Abschnitt 5). Deshalb aktiviert ihn auch kein Paket, sondern jeder Mensch für
sich, mit einem Befehl oder einem Klick in der Einrichtung.

## Voraussetzungen

- Linux mit systemd und einer Benutzersitzung (`systemctl --user` antwortet).
- bubblewrap 0.8 oder neuer und unprivilegierte Nutzer-Namensräume.
- Für die Anwendung GTK 3, GLib und eine Grafikanbindung (EGL, GLES 2).

`humanitl doctor` prüft alles davon und nennt zu jedem Befund den Befehl, der ihn behebt.

## Paket (.deb)

Aus dem APT-Repository `apt.nurkert.de`, das jedes Release übernimmt; spätere Versionen kommen dann
mit `apt upgrade`:

```sh
curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh
humanitl daemon install
humanitl daemon status
```

Das Skript legt den Signaturschlüssel des Repositorys nach
`/usr/share/keyrings/nurkert-archive-keyring.gpg`, die Quelle nach
`/etc/apt/sources.list.d/nurkert.list` und installiert das Paket. Wer einer Pipe in eine
Root-Shell nicht traut, liest es vorher oder nimmt das Paket von der Release-Seite:

```sh
sudo apt install ./humanitl_<version>_amd64.deb
humanitl daemon install
humanitl daemon status
```

Das Paket legt die Anwendung und ihre Programme nach `/usr/lib/humanitl/`, Verweise nach
`/usr/bin/humanitl` und `/usr/bin/humanitl-app`, Desktop-Eintrag und Symbole nach `/usr/share/`
und die beiden Units `humanitld.service` und `humanitld.socket` nach `/usr/lib/systemd/user/`.
Unter `/etc`, `/var` und im Heimatverzeichnis legt es nichts an, und es hat keine
Maintainer-Skripte: Menü und Symbol-Cache aktualisieren die Trigger von `desktop-file-utils` und
`hicolor-icon-theme`.

`humanitl daemon install` erkennt die Units des Pakets und **schreibt dann nichts**. Es führt nur aus,
was das Paket als root nicht kann:

```sh
systemctl --user daemon-reload
systemctl --user enable --now humanitld.socket humanitld.service
```

Beide Units, nicht nur der Socket: Ein Client liest das Token des Daemons, bevor er den Socket
öffnet, und das Token schreibt erst der laufende Daemon. Ein Socket allein weckte den Dienst also
nie. Die Socket-Unit hält den Pfad `$XDG_RUNTIME_DIR/humanitl/daemon.sock` über jeden Neustart des
Dienstes hinweg; wer in dieser Zeit verbindet, wartet, statt abzuprallen.

Wer eine Einstellung der Unit ändern will, kopiert sie nicht, sondern legt eine Ergänzung an:
`systemctl --user edit humanitld.service`. Eine Kopie unter `~/.config/systemd/user/` verdeckte
die Fassung des Pakets, und jedes Update liefe an ihr vorbei.

Wer vorher aus dem Archiv oder dem AppImage installiert hatte, hat dort noch die Unit, die
`humanitl daemon install` damals geschrieben hat. Der Befehl erkennt sie an ihrer ersten Zeile,
kündigt es an, legt sie samt ihrem Verweis der Aktivierung als `humanitld.service.bak` beiseite (gibt
es den Namen schon, als `humanitld.service.bak.1`, `.bak.2` und so weiter) und startet den Dienst neu,
damit die Unit des Pakets läuft. Unter `--no-start` oder ohne `systemctl` bleibt sie liegen, und die
Ausgabe sagt, dass sie die Unit des Pakets weiter verdeckt. Das Beiseitelegen nutzt
`renameat2` mit `RENAME_NOREPLACE`; liegt `~/.config` auf NFS, das diesen Aufruf mit `EINVAL`
ablehnt, bricht der Befehl mit `DAEMON_006` ab und lässt alles, wie es war. Eine Datei ohne diese
erste Zeile rührt er nicht an und bricht mit `DAEMON_005` ab: Sie gehört dann jemand anderem, und
wer sie nicht mehr braucht, räumt sie selbst weg oder überführt seine Änderungen in
`systemctl --user edit`.

Entfernen: `humanitl daemon uninstall` (meldet Socket und Dienst ab, dasselbe wie
`systemctl --user disable --now humanitld.socket humanitld.service`), dann
`sudo apt remove --purge humanitl`. Konfiguration, Aufzeichnung und Audit-Log unter
`~/.config/humanitl` und `~/.local/share/humanitl` bleiben liegen; sie gehören dem Menschen, nicht
dem Paket.

## AppImage

```sh
chmod +x Humanitl-<version>-x86_64.AppImage
./Humanitl-<version>-x86_64.AppImage                       # die Anwendung
./Humanitl-<version>-x86_64.AppImage --cli daemon install  # der Dienst
```

`--cli` reicht alle weiteren Argumente an die Kommandozeile im Bild weiter. `daemon install`
erkennt das AppImage an `$APPIMAGE` und kopiert Daemon und Shim nach
`~/.local/lib/humanitl/<version>.<stempel>/`, denn der Einhängepunkt `/tmp/.mount_*` verschwindet
mit dem Prozess. `ExecStart` der Unit nennt den Verweis `~/.local/lib/humanitl/current`.
Statt des zweiten Befehls geht auch der Knopf „Installieren und starten" in der Einrichtung der
Anwendung; er ruft dieselbe Kommandozeile.

**Eine neue Fassung erneuert den Dienst beim Start.** Wer später ein neueres AppImage startet,
muss nichts tun: `AppRun` ruft vor der Anwendung `daemon install --refresh`. Zeigt `current` auf
eine andere Fassung, kopiert es Daemon und Shim der neuen heraus, hängt `current` um, startet den
Dienst mit `systemctl --user restart humanitld.service` neu und entfernt erst danach die alte
Kopie. Ist der Dienst nicht eingerichtet oder schon diese Fassung, tut der Aufruf nichts. Die erste
Einrichtung macht ein Programmstart nie von selbst.

Entfernen:

```sh
./Humanitl-<version>-x86_64.AppImage --cli daemon uninstall --purge-binaries
```

Das meldet den Dienst ab und entfernt die Unit, ihre Verweise, einen liegengebliebenen Socket und
alles unter `~/.local/lib/humanitl/`, was `daemon install` dort angelegt hat. Danach zeigt
`humanitl doctor` die Zeile `daemon` mit `DOCTOR_006` und dem Vorschlag, den Dienst wieder
einzurichten.

Das Bild enthält nur das Flutter-Bundle und die drei Programme. GTK, GLib, Mesa und die
Wayland-Bibliotheken kommen vom System; ein gebündeltes GTK bricht auf Wayland und bei
abweichenden Mesa-Fassungen. Auf einem System ohne `libayatana-appindicator3` läuft die
Anwendung, nur das Tray-Symbol fehlt.

Ein AppImage braucht zum Einhängen `libfuse2`, das neue Distributionen nicht mehr mitbringen.
Ohne FUSE startet es mit `--appimage-extract-and-run`, das den Inhalt in ein temporäres
Verzeichnis entpackt:

```sh
./Humanitl-<version>-x86_64.AppImage --appimage-extract-and-run
```

## Archiv

```sh
tar -xzf humanitl-<version>-linux-x86_64.tar.gz
humanitl-<version>-linux-x86_64/bin/humanitl daemon install
```

`daemon install` schreibt dann `~/.config/systemd/user/humanitld.service` mit dem Pfad des Daemons
im Archiv und aktiviert sie. Eine Socket-Unit gibt es auf diesem Weg nicht; der Daemon bindet den
Socket selbst. Die Datei trägt in der ersten Zeile die Marke von `daemon install`, und nur eine
Datei mit dieser Marke ersetzt der Befehl je (`DAEMON_005`). Entfernen:
`humanitl-<version>-linux-x86_64/bin/humanitl daemon uninstall`, dann das Verzeichnis.

## Einrichtung in der Anwendung

Die erste Zeile der Einrichtung fragt, ob der Daemon antwortet. Antwortet keiner, bietet sie
„Installieren und starten" an; der Knopf ruft `humanitl daemon install` neben der laufenden
Anwendung auf, ohne Shell und nie aus `PATH`, und die Zeile wird grün, sobald der Dienst antwortet.
Findet die Anwendung keine Kommandozeile neben sich, zeigt sie den Befehl zum Kopieren.

<a id="hardening"></a>

## Härtung der Unit

`packaging/systemd/humanitld.service` ist so weit gehärtet, wie die Sandbox es zulässt. Jede Zeile
ist gemessen, am 2026-09-18 auf Debian 14 mit systemd 262 und bubblewrap 0.12.0, mit
`systemd-run --user` und denselben Eigenschaften: Die Escape-Tests (`tests/escape/run.sh`)
laufen darunter mit 124 von 124 Fällen grün, darunter die drei Sandbox-Garantien und das Terminal
des Agenten; ein schreibbares Projekt unter `$HOME` ist in der Sandbox schreibbar; node, python3 und
java laufen darunter. Das ist wichtig, weil **jedes Kind des Daemons die Einschränkungen der Unit erbt**:
das Sandbox-Backend, der Shim und der Agent. Was hier steht, gilt für den Agenten mit.

`systemd-analyze --user security --offline=true` auf die Unit mit dem Pfad des Pakets:

Overall exposure level: 3.7 (systemd 262, Debian 14, 2026-09-18; Grenze aus HUM-053: 4.0)

Der Test `the_exposure_stays_at_or_below_the_documented_value` in
`daemon/bin/humanitl/src/cmd/unit.rs` misst den Wert bei jedem Lauf nach, wo `systemd-analyze`
das kann, und hält ihn bei 4,0 oder darunter.

### Was gesetzt ist

| Zeile | Grund |
|---|---|
| `Type=notify` | Der Daemon meldet `READY=1`, sobald Token und Socket stehen. |
| `NoNewPrivileges=yes` | bubblewrap läuft ohne setuid über Nutzer-Namensräume und braucht keine neuen Rechte. |
| `PrivateTmp=yes` | Das `/tmp` des Daemons ist seines. Ein Projekt unter `/tmp` sieht er deshalb nicht. |
| `ProtectSystem=strict` | In einer Benutzer-Unit ist danach alles schreibgeschützt außer `$HOME`, dem Laufzeitverzeichnis und den API-Dateisystemen (gemessen, nicht gelesen). |
| `ReadWritePaths=-%h/.local/share/humanitl -%h/.config/humanitl -%t/humanitl` | Die drei Verzeichnisse des Daemons, jeweils mit `-`, weil systemd eine Unit mit einem fehlenden Pfad nicht startet. |
| `PrivateIPC=yes`, `PrivateMounts=yes`, `PrivateUsers=yes` | Eigene IPC- und Mount-Namensräume, nur der eigene Nutzer sichtbar. |
| `ProtectClock=yes`, `ProtectControlGroups=yes`, `ProtectKernelModules=yes` | Uhr, cgroups und Kernelmodule sind nicht Sache des Daemons. |
| `ProtectProc=invisible` | Prozesse anderer Nutzer sind unsichtbar. |
| `KeyringMode=private`, `LockPersonality=yes`, `RestrictRealtime=yes` | Standard-Härtung ohne Wirkung auf die Sandbox. |
| `RestrictFileSystems=~@privileged-api` | Keine Dateisysteme wie `bpf` oder `debugfs`. |
| `CapabilityBoundingSet=CAP_SYS_ADMIN` | Ein Benutzerdienst hat keine Capabilities; die Grenze wirkt auf Programme mit Datei-Capabilities. Leer darf sie nicht sein (siehe unten). |
| `SystemCallFilter=@system-service @mount @sandbox sethostname`, `SystemCallErrorNumber=EPERM` | `@mount`, weil bubblewrap mountet und `pivot_root`t; `@sandbox`, weil der Shim seinen eigenen seccomp-Filter mit `seccomp(2)` setzt; `sethostname`, weil bubblewrap den UTS-Namensraum der Sandbox `sandbox` nennt („Can't set hostname to sandbox" ohne ihn). Den Namen des Rechners ändert das nicht. |
| `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK` | `AF_NETLINK` braucht bubblewrap für `lo` im Namensraum, IPv4 und IPv6 der Proxy für den Weg zum Ziel. |

### Was mit Absicht fehlt

| Zeile | Gemessene Folge |
|---|---|
| `RestrictNamespaces` | bubblewrap: „No permissions to create a new namespace". Die Sandbox braucht user, mnt, pid, net, ipc und uts. |
| `ProtectKernelTunables=yes` | bubblewrap: „Can't mount proc on /proc: Operation not permitted". Ein überdecktes `/proc/sys` verbietet ein neues `/proc`. |
| `ProtectKernelLogs=yes` | Derselbe Fehler, wegen des überdeckten `/proc/kmsg`. |
| `ProtectHostname=yes` | Allein harmlos, zusammen mit den übrigen Zeilen derselbe Fehler beim Einhängen von `/proc`. |
| `RestrictSUIDSGID=yes` | bubblewrap: „Can't open source /usr: Function not implemented". |
| `MemoryDenyWriteExecute=yes` | Vererbt auf den Agenten: node (V8) bricht beim Start mit „Check failed: 12 == (*__errno_location ())" ab, ebenso jede andere JIT-Laufzeit. |
| `CapabilityBoundingSet=` (leer) | bubblewrap startet nicht. |
| `SystemCallArchitectures=native` | Der Shim prüft seinen eigenen Filter mit einem x32-Aufruf von `socket` und stirbt dabei an `SIGSYS`; die Sandbox startet nicht (`SANDBOX_016`, „families not reported"). |
| `PrivateDevices=yes` | Ohne `/dev/ptmx` öffnet der Daemon kein Terminal für den Agenten (`TERM_002`, „cannot open a pseudo terminal"). |
| `UMask=0077` | Vererbt auf den Agenten: Jede Datei, die er im Projekt anlegt, bekäme andere Rechte als ohne Humanitl. |
| `ProtectHome=read-only` | Ein Projekt unter `$HOME` wäre in der Sandbox nur lesbar; `sandbox.work_mode = rw` ist die Vorgabe. |
| `ProcSubset=pid` | `humanitl doctor` liest `/proc/sys/kernel/*` und `/proc/modules`; die Befunde dazu fielen weg. |
| `PrivateNetwork`, `IPAddressDeny` | Der Proxy muss das Ziel erreichen, das der Mensch freigibt. |

### Projekte außerhalb von `$HOME`

Unter `ProtectSystem=strict` ist ein Projekt außerhalb des Heimatverzeichnisses, etwa unter
`/srv` oder einem eigenen Datenträger, für den Daemon und damit in der Sandbox nur lesbar. Freigeben
lässt es sich mit einer Ergänzung der Unit:

```sh
systemctl --user edit humanitld.service
# [Service]
# ReadWritePaths=/srv/mein-projekt
systemctl --user restart humanitld.service
```

Ein Projekt unter `/tmp` sieht der Daemon wegen `PrivateTmp` gar nicht.

## XDG_RUNTIME_DIR ohne logind

Socket und Token des Daemons liegen im Laufzeitverzeichnis `$XDG_RUNTIME_DIR/humanitl`. Fehlt
`XDG_RUNTIME_DIR` und gibt es kein `/run/user/<uid>`, etwa ohne logind, weichen Daemon, CLI und
Oberfläche auf `$TMPDIR/humanitl-<uid>` aus. Diesen Namen kann jedes andere Konto vorab anlegen.
Humanitl weist ein solches fremdes Verzeichnis ab (`DAEMON_001`, `DAEMON_004`) und startet dann
nicht, solange es dort liegt (`docs/SECURITY.md` Abschnitt 10, Punkt 12).

Abhilfe ist ein eigenes Laufzeitverzeichnis unter dem Heimatverzeichnis. Daemon, CLI und
Oberfläche müssen dieselbe Variable sehen; sie gehört deshalb in die Anmeldung und nicht vor
einen einzelnen Befehl. Das Verzeichnis einmal anlegen:

```sh
mkdir -p "$HOME/.cache/humanitl-run"
chmod 700 "$HOME/.cache/humanitl-run"
```

Dann die folgende Zeile in die Datei eintragen, die die eigene Anmeldung liest. Sie setzt die
Variable nur, wenn sie fehlt und `/run/user/<uid>` nicht existiert; eine spätere Sitzung mit
logind behält so ihre eigenen Sockets für Wayland, PipeWire und D-Bus.

```sh
[ -n "$XDG_RUNTIME_DIR" ] || [ -d "/run/user/$(id -u)" ] || export XDG_RUNTIME_DIR="$HOME/.cache/humanitl-run"
```

Welche Datei das ist, hängt von der Shell ab:

- `bash` liest bei der Anmeldung nur die erste vorhandene von `~/.bash_profile`,
  `~/.bash_login` und `~/.profile`. Die Zeile gehört in genau diese Datei. `~/.profile` nur dann
  neu anlegen, wenn weder `~/.bash_profile` noch `~/.bash_login` existiert.
- `zsh` liest `~/.zprofile`, nicht `~/.profile`. Die Zeile gehört nach `~/.zprofile`; die Datei
  darf dafür neu angelegt werden.
- Display-Manager ohne systemd-Sitzung lesen meist `~/.profile`. Wer die Oberfläche von dort
  startet, trägt die Zeile zusätzlich dort ein.

Die Zeile wirkt erst nach einer neuen Anmeldung. Danach finden Daemon, CLI und Oberfläche
dasselbe Verzeichnis. Dass die Clients es ohne diese Zeile selbst finden, ist HUM-222.

## Paket bauen

```sh
make package PACKAGE_VERSION=0.0.N
```

Das Ziel baut `humanitld`, `humanitl` und den statischen Shim
(`packaging/release/build-binaries.sh`) nach `app/linux/bundle-extra/`, dann das Flutter-Bundle
(`app/linux/CMakeLists.txt` legt die drei nach `bin/`), dann
`dist/humanitl_<version>_amd64.deb` (`packaging/deb/build-deb.sh`) und
`dist/Humanitl-<version>-x86_64.AppImage` (`packaging/appimage/build-appimage.sh`), und prüft das
AppImage (`packaging/appimage/check-appimage.sh`). Gebraucht werden `dpkg-dev`, `patchelf`,
`curl` und für den statischen Shim `musl-tools`. `appimagetool` und die Laufzeit des AppImage lädt
das Skript in festgelegter Fassung und prüft ihre SHA-256-Summe, bevor es sie ausführt.

Die PNG-Symbole unter `packaging/deb/icons/` sind aus `packaging/deb/humanitl.svg` gerechnet:

```sh
for s in 64 128 256; do
  inkscape packaging/deb/humanitl.svg --export-type=png --export-width=$s \
    --export-height=$s --export-filename=packaging/deb/icons/humanitl-$s.png
done
```

Das `.deb` wird nie auf einem Arbeitsrechner installiert, sondern in einem Wegwerf-Container
geprüft, mit `packaging/deb/check-install.sh` (Installation, Bibliotheken, Start der Anwendung
unter Xvfb, Units, Symbole, `daemon install --print`, Entfernen mit `--purge`) und
`packaging/deb/lintian.sh` (keine Fehler, keine Warnungen; jede Ausnahme steht mit Grund in
`packaging/deb/lintian-overrides`).

## Test-Matrix (manuell)

Was kein Container zeigt, prüft ein Mensch auf echter Hardware, je Vorabversion und vor 0.1.0.
Ergebnis und Datum stehen in der Tabelle; ein leeres Feld heißt: noch nicht geprüft.

| Prüfung | Debian 13 GNOME Wayland (Intel) | Debian 13 KDE Wayland (NVIDIA proprietär) | Ubuntu 24.04 GNOME X11 |
|---|---|---|---|
| `sudo apt install ./humanitl_…deb` auf frischer VM, dann `humanitl daemon install` und `humanitl daemon status` mit Exit 0 | | | |
| Anwendung startet, Einrichtung zeigt den Daemon grün | | | |
| Tray-Symbol (GNOME braucht die Erweiterung „AppIndicator and KStatusNotifierItem Support"; die Einrichtung weist darauf hin) | | | |
| Fenster bei 200 % Skalierung scharf und vollständig | | | |
| Benachrichtigung bei einer gehaltenen Anfrage | | | |
| Sandbox-Start mit schreibbarem Projekt unter `$HOME` unter der Unit | | | |
| Impeller rendert (Fenster nicht schwarz, `humanitl doctor` ohne `DOCTOR_010`) | | | |
| AppImage startet ohne gebündeltes GTK | | | |

Fällt Impeller auf einem Treiber aus (schwarzes Fenster, Absturz beim Start), gehört der Befund
mit Treiber und Fassung in diese Tabelle. Einen Rückfall auf einen anderen Renderer gibt es im
Release-Bau noch nicht: Schalter der Flutter-Engine wie `--no-enable-impeller` reicht der
Linux-Runner nicht an die Engine weiter, sondern an den Dart-Code, und die Umgebungsvariablen der
Engine gelten nach ihrem Quelltext nur außerhalb von Release-Bauten. Gemessen ist beides hier
nicht (HUM-168).
