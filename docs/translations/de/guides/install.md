---
source: guides/install.md
source-sha256: f8e858a6bc1d5aadc5ba337c77076fad4eae59e27bcd93313636970e7ae62f38
lang: de
---
# Humanitl installieren

Applies to: Humanitl 0.1

Diese Anleitung bringt einen Linux-Desktop-Rechner vom Nullzustand zu einem
laufenden Humanitl-Daemon mit bestandener Sandbox-Prüfung. Das dauert etwa zehn
Minuten. Der nächste Schritt danach ist [deine erste Session](first-session.md).

Humanitl gibt es für x86_64 in fünf Formen. Alle enthalten dieselben vier
Programme: die Desktop-App, die Kommandozeile `humanitl`, den Daemon `humanitld`
und den statischen Shim `humanitl-shim`, der in der Sandbox läuft.

| Form | Datei | Am besten für |
|---|---|---|
| Debian-Paket | `humanitl_<version>_amd64.deb` | Debian und Ubuntu; Updates über apt |
| RPM-Paket | `humanitl-<version>-1.x86_64.rpm` | Fedora 40 und neuer, openSUSE Tumbleweed |
| AUR-Paket | `humanitl-bin`, aus dem Archiv gebaut | Arch Linux und seine Ableger, sobald es veröffentlicht ist |
| AppImage | `Humanitl-<version>-x86_64.AppImage` | andere Distributionen oder zum Ausprobieren ohne root |
| Archiv | `humanitl-<version>-linux-x86_64.tar.gz` | Server und skriptgesteuerte Einrichtungen ohne die App |

Flatpak, Snap und arm64 gehören nicht zu 0.1, ein dnf-Repository ebenso wenig:
Das RPM wird aus der Datei installiert, wie das `.deb` ohne das APT-Repository.

Der Daemon läuft immer als **systemd-Benutzerdienst** des Menschen, der ihn
nutzt, nie als Systemdienst und nie mit `sudo`. Er liest das
Laufzeitverzeichnis, die Konfiguration und die Aufzeichnung eines einzigen
Menschen und von niemandem sonst. Deshalb schaltet ihn auch kein Paket von sich
aus ein: Jeder schaltet ihn einmal selbst ein, mit einem Befehl oder einem Klick
in der App.

## Voraussetzungen

| Was | Warum | So prüfst du es |
|---|---|---|
| Debian 13 oder Ubuntu 24.04, oder neuer, auf x86_64; für das RPM Fedora 40 oder neuer oder openSUSE Tumbleweed; für das AUR-Paket ein aktuelles Arch Linux | die Programme werden auf Ubuntu 24.04 gebaut und brauchen glibc 2.39 oder neuer, was RHEL 9 (2.34) und openSUSE Leap 15 (2.31) nicht haben | `cat /etc/os-release`, `ldd --version` |
| systemd mit einer Benutzersitzung | der Daemon ist ein Benutzerdienst | `systemctl --user is-system-running` antwortet `running` |
| bubblewrap 0.8.0 oder neuer | das Sandbox-Backend; ältere Versionen werden zurückgewiesen (`SANDBOX_002`) | `bwrap --version` |
| unprivilegierte User-Namespaces | bubblewrap öffnet die Sandbox ohne setuid | siehe [User-Namespaces](#user-namespaces) |
| ein Kernel mit seccomp, 5.4 oder neuer | die dritte Garantie, keine neuen Sockets | `grep Seccomp /proc/self/status` |
| GTK 3, GLib, EGL und GLES 2 | die Desktop-App | das `.deb`, das `.rpm` und das AUR-Paket ziehen sie nach |
| `libayatana-appindicator3-1` (optional) | das Tray-Symbol; ohne sie läuft die App, und der Fenstertitel trägt die Zahl der angehaltenen Anfragen | `humanitl doctor`, Zeile `tray` |
| `libsecret-tools` (empfohlen) | bewahrt einen Zugangsschlüssel für den Modellserver im Schlüsselbund des Systems auf; ohne es liegt der Schlüssel in einer Datei mit Modus `0600` | `command -v secret-tool` |

Das `.deb` gibt bubblewrap, EGL und GLES 2 als Abhängigkeiten an, also
installiert apt sie; das `.rpm` und das AUR-Paket geben dasselbe an, für dnf,
zypper und pacman. GTK 3 und GLib kommen über die Bibliotheken der App mit. Die
Tray-Bibliothek und `libsecret-tools` sind optional; das Paket empfiehlt
`libsecret-tools`.
Unter macOS braucht es dafür nichts: Der Zugangsschlüssel liegt in deinem
Anmelde-Schlüsselbund, über das Systemwerkzeug `/usr/bin/security`.

Unter GNOME braucht das Tray-Symbol zusätzlich die Shell-Erweiterung
„AppIndicator and KStatusNotifierItem Support" (Debian:
`gnome-shell-extension-appindicator`). Der Einrichtungsbildschirm der App weist
darauf hin, wenn sie fehlt.

Du musst das alles nicht von Hand prüfen. Nach der Installation gibt
`humanitl doctor` eine Zeile je Voraussetzung aus, und jede Zeile, die nicht
`ok` ist, trägt den Befehl, der sie behebt.

### User-Namespaces

Humanitl startet die Sandbox mit bubblewrap als gewöhnlicher Benutzer. Dafür
braucht es unprivilegierte User-Namespaces. Debian und Ubuntu erlauben sie
standardmäßig, mit zwei Ausnahmen, die du kennen solltest:

- **Ein Kernel-Schalter von Debian.** Debian-Kernel tragen den Schalter
  `kernel.unprivileged_userns_clone`. Steht er auf `0`, kann niemand einen
  User-Namespace öffnen. Prüfen und beheben:

  ```sh
  sysctl kernel.unprivileged_userns_clone
  sudo sysctl -w kernel.unprivileged_userns_clone=1
  ```

  Auch die allgemeine Grenze `user.max_user_namespaces` darf nicht `0` sein
  (`sysctl user.max_user_namespaces`).

- **Die AppArmor-Einschränkung von Ubuntu 23.10 und neuer.** Ubuntu schränkt
  unprivilegierte User-Namespaces über AppArmor ein
  (`kernel.apparmor_restrict_unprivileged_userns = 1`). Das `bwrap` aus dem
  Ubuntu-Archiv trägt ein AppArmor-Profil und darf trotzdem; ein bubblewrap von
  anderswo darf es nicht. Schlägt die Doctor-Zeile `userns` fehl und nennt sie
  diesen Schalter, nimm entweder das `bwrap` der Distribution oder schalte die
  Einschränkung ab, im Wissen, dass das sie für jedes Programm aufhebt:

  ```sh
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
  ```

  `sysctl -w` hält bis zum nächsten Systemstart. Damit eine Einstellung bleibt,
  schreib sie in eine Datei unter `/etc/sysctl.d/`.

`humanitl doctor` misst das mit bubblewrap selbst (`bwrap --unshare-user`),
nicht mit `unshare`, weil das der Weg ist, den die Sandbox nimmt. Seine Zeile
`userns` nennt den Schalter, der im Weg steht, und den Befehl dafür
(`DOCTOR_002`).

## Das Debian-Paket installieren

Lade `humanitl_<version>_amd64.deb` von der Release-Seite des Projekts herunter
und installiere es mit apt, das die Abhängigkeiten auflöst:

```sh
sudo apt install ./humanitl_<version>_amd64.deb
humanitl daemon install
humanitl daemon status
```

Ein APT-Repository, das jedes Release trägt, beschreiben die
[Notizen für Maintainer](../INSTALL.md#apt-repository).

Was das Paket wohin legt:

| Pfad | Inhalt |
|---|---|
| `/usr/lib/humanitl/` | das App-Bundle und unter `bin/` die Programme `humanitld`, `humanitl` und `humanitl-shim` |
| `/usr/bin/humanitl` | Verweis auf die Kommandozeile |
| `/usr/bin/humanitl-app` | Verweis auf die Desktop-App |
| `/usr/lib/systemd/user/humanitld.service`, `humanitld.socket` | die Benutzer-Units |
| `/usr/share/humanitl/catalog/` | der Domain-Katalog |
| `/usr/share/humanitl/profiles/sandbox/default.toml` | das Sandbox-Profil |
| `/usr/share/applications/`, `/usr/share/icons/hicolor/` | Menüeintrag und Symbole |

Das Paket schreibt nichts unter `/etc`, `/var` oder in dein Home-Verzeichnis.
Seine einzigen Paketskripte laden nach einer Installation, einem Upgrade oder
einer Entfernung die Benutzer-Units angemeldeter Benutzer neu und starten einen
laufenden Daemon neu, der keine Agenten-Session hat (siehe [Update](#update)
und `docs/INSTALL.md`).

### Den Benutzerdienst einschalten

`humanitl daemon install` erkennt die Units des Pakets und **schreibt nichts**.
Es führt nur aus, was das Paket als root nicht kann:

```sh
systemctl --user daemon-reload
systemctl --user enable --now humanitld.socket
```

Eingeschaltet wird nur der Socket, nicht der Dienst. systemd startet den Daemon
bei der ersten Verbindung, aus der App oder von der Kommandozeile, und der
Socket-Pfad `$XDG_RUNTIME_DIR/humanitl/daemon.sock` bleibt über Neustarts des
Dienstes hinweg bestehen. Ein Client, der noch kein Token des Daemons findet,
wartet bis zu zehn Sekunden, bis der Dienst hochgekommen ist, endet dann mit
`DAEMON_001` und verweist auf `humanitl daemon logs`. Soll der Daemon stattdessen
mit jeder Anmeldung starten, führe zusätzlich
`systemctl --user enable humanitld.service` aus.

Willst du eine Einstellung der Unit ändern, kopiere sie nicht. Lege mit
`systemctl --user edit humanitld.service` ein Drop-in an; eine Kopie unter
`~/.config/systemd/user/` würde die Unit des Pakets vor jedem Update verdecken.
Warum die Unit so gehärtet ist, wie sie ist, und wie du dem Daemon ein
beschreibbares Projekt außerhalb deines Home-Verzeichnisses gibst, steht in den
[Notizen für Maintainer](../INSTALL.md#hardening).

Statt des Befehls bietet die App denselben Schritt an: Die erste Zeile ihres
Einrichtungsbildschirms, „Hintergrunddienst", zeigt den Knopf „Installieren und
starten", wenn kein Daemon antwortet. Er führt das `humanitl` neben der App aus,
nie eines aus `PATH`, und wird grün, sobald der Dienst antwortet.

## Das RPM installieren

Für Fedora 40 und neuer und openSUSE Tumbleweed. Das RPM enthält dieselben
Dateien unter denselben Pfaden wie das `.deb` (siehe die Tabelle oben); nur die
Lizenztexte liegen unter `/usr/share/licenses/humanitl/`. Der Release-Lauf
installiert, prüft und entfernt es in einem Fedora-44-Container; Tumbleweed wird
nicht getestet. Ältere Systeme wie RHEL 9 oder openSUSE Leap 15 weisen es
zurück, weil ihre glibc älter als 2.39 ist.

```sh
sudo dnf install ./humanitl-<version>-1.x86_64.rpm
humanitl daemon install
humanitl daemon status
```

Unter openSUSE ist das Release nicht signiert, deshalb verlangt zypper das
ausdrücklich:

```sh
sudo zypper install --allow-unsigned-rpm ./humanitl-<version>-1.x86_64.rpm
```

Wie das `.deb` schreibt das Paket nichts unter `/etc`, `/var` oder in dein
Home-Verzeichnis, und `humanitl daemon install` schaltet nur den mitgelieferten
Socket ein (siehe [Den Benutzerdienst einschalten](#enable-the-user-service)).
Seine Scriptlets laden nach einer Installation, einem Upgrade oder einer
Entfernung die Benutzer-Units angemeldeter Benutzer neu und starten einen
laufenden Daemon neu, der keine Agenten-Session hat.

## Aus dem AUR installieren

Das AUR-Paket `humanitl-bin` nimmt das Archiv
`humanitl-<version>-linux-x86_64.tar.gz` eines Release, prüft dessen SHA-256 und
installiert es mit demselben Aufbau wie das `.deb`, Units und Menüeintrag
eingeschlossen. Der Release-Lauf baut es mit makepkg und installiert und
entfernt es in einem Arch-Linux-Container. Der Maintainer veröffentlicht es nach
jedem Release von Hand im AUR, deshalb kann es eine Weile hinter der
Release-Seite zurückliegen; solange
<https://aur.archlinux.org/packages/humanitl-bin> nicht existiert, ist das Paket
noch nicht veröffentlicht.

```sh
git clone https://aur.archlinux.org/humanitl-bin.git
cd humanitl-bin
less PKGBUILD humanitl-bin.install   # read what you are about to run as root
makepkg -si
humanitl daemon install
humanitl daemon status
```

Ein AUR-Helfer wie `yay -S humanitl-bin` tut dasselbe. Das Paket braucht
`bubblewrap` 0.8.0 oder neuer aus den Arch-Repositories; `libsecret` und
`libayatana-appindicator` sind optional.

## Das AppImage installieren

```sh
chmod +x Humanitl-<version>-x86_64.AppImage
./Humanitl-<version>-x86_64.AppImage                       # the app
./Humanitl-<version>-x86_64.AppImage --cli daemon install  # the service
./Humanitl-<version>-x86_64.AppImage --cli daemon status
```

`--cli` reicht alles dahinter an die Kommandozeile im Image weiter.
`daemon install` erkennt, dass es aus einem AppImage läuft (`$APPIMAGE`), und
kopiert den Daemon, den Shim, den Domain-Katalog und das Sandbox-Profil nach
`~/.local/lib/humanitl/<version>.<stamp>/`, weil der Einhängepunkt des Images
mit dem Prozess verschwindet. Danach schreibt es
`~/.config/systemd/user/humanitld.service`, das
`~/.local/lib/humanitl/current/bin/humanitld` startet, und schaltet es ein. Der
Knopf „Installieren und starten" des Einrichtungsbildschirms tut dasselbe.

Ein neueres AppImage frischt den Dienst von selbst auf: Jeder Start des Images
führt zuerst `daemon install --refresh` aus, das die neue Version kopiert,
`current` umstellt und den Dienst neu startet, aber nur, wenn der Dienst vorher
eingerichtet war. Die erste Einrichtung geschieht nie dadurch, dass du die App
einfach startest.

Das Image enthält nur das App-Bundle und die Programme. GTK, GLib, Mesa und die
Wayland-Bibliotheken kommen von deinem System. Ein AppImage braucht `libfuse2`,
um sich selbst einzuhängen (Ubuntu 24.04: `libfuse2t64`). Ohne FUSE startest du
es mit `--appimage-extract-and-run`.

## Das Archiv installieren

```sh
tar -xzf humanitl-<version>-linux-x86_64.tar.gz
humanitl-<version>-linux-x86_64/bin/humanitl daemon install
humanitl-<version>-linux-x86_64/bin/humanitl daemon status
```

`daemon install` schreibt `~/.config/systemd/user/humanitld.service` mit dem
Pfad des Daemons im entpackten Verzeichnis und schaltet es ein. Auf diesem Weg
gibt es keine Socket-Unit; der Daemon bindet seinen Socket selbst. Lass das
Verzeichnis, wo es ist, oder führe `daemon install` nach dem Verschieben erneut
aus.

## Unter macOS installieren

Humanitl wird für macOS 12 oder neuer gebaut, auf Apple Silicon und Intel; der
Release-Lauf testet es unter macOS 15, und benutzt wurde es unter macOS 27. Auf
anderen Versionen sagt dir die Isolationsprüfung bei jedem Start, ob die Sandbox
hält. Das Release enthält `Humanitl-<version>-macos.dmg` und dieselbe App als
`Humanitl-<version>-macos.zip`.

1. Öffne das Disk-Image und zieh Humanitl in den Ordner „Programme".
2. Die App ist ad hoc signiert, nicht mit einer Apple Developer ID, deshalb
   verweigert macOS den ersten Start. Erlaube ihn einmal unter
   Systemeinstellungen › Datenschutz & Sicherheit › Dennoch öffnen oder in einem
   Terminal:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Humanitl.app
   ```

   Starte die App aus „Programme", nicht aus dem Disk-Image oder aus
   „Downloads": macOS führt eine App unter Quarantäne aus einer temporären Kopie
   aus, und der Dienst würde auf diese Kopie zeigen.
3. Auf dem Einrichtungsbildschirm führt „Dienst einrichten"
   `humanitl daemon install` aus. Es schreibt
   `~/Library/LaunchAgents/dev.humanitl.humanitld.plist`, und launchd startet
   den Daemon jetzt und bei jeder Anmeldung. Sein Log ist
   `~/Library/Logs/Humanitl/humanitld.log`.

Die Kommandozeile liegt in der App; verlinke sie in deinen `PATH`, wenn du sie
oft benutzt:

```sh
ln -s /Applications/Humanitl.app/Contents/MacOS/bin/humanitl /usr/local/bin/humanitl
humanitl daemon status
humanitl sandbox check        # three green lines
humanitl daemon restart       # launchctl kickstart, after an update
humanitl daemon uninstall     # removes the launch agent
```

Socket und Token liegen in `~/Library/Application Support/Humanitl/run`. Die
Sandbox ist Seatbelt, nicht bubblewrap; dafür gibt es nichts zu installieren.
Was sie unter macOS garantiert, steht in
[`SECURITY.md` Abschnitt 12](../SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap).
Der Agent muss aus der Sandbox heraus lesbar sein, das Home-Verzeichnis ist es
nicht: Ein einzelnes natives Programm wie das von OpenCode darf überall liegen
(der Start erlaubt diese eine Datei), aber ein Agent mit Begleitdateien, etwa pi
mit seinen Node-Modulen, muss außerhalb des Home-Verzeichnisses installiert
sein, über `npm install -g` mit dem Node von Homebrew. `brew install opencode`
geht ebenso.

## Den Agenten installieren

Humanitl 0.1 bringt zwei Agenten-Adapter mit, OpenCode (die Vorgabe) und pi.
Installiere OpenCode so, wie sein Projekt es beschreibt, zum Beispiel:

```sh
curl -fsSL https://opencode.ai/install | bash
```

Die Sandbox hängt `/usr` nur lesbar ein und sucht in
`PATH=/usr/local/bin:/usr/bin:/bin`. Ein Programm unter deinem
Home-Verzeichnis, etwa `~/.opencode/bin/opencode`, ist darin nicht sichtbar.
Leg eine Kopie dorthin, wo die Sandbox sie findet:

```sh
sudo install -m 0755 ~/.opencode/bin/opencode /usr/local/bin/opencode
```

Lässt du das aus, bricht der Start einer Session mit `AGENT_004` ab und nennt
genau diesen Befehl. Die [Anleitung zu Agentenprofilen](agent-profiles.md)
erklärt, was Humanitl für OpenCode einstellt und wie du ein anderes Programm als
Agenten laufen lässt.

Willst du stattdessen pi nutzen, installiere es und Node.js 22.19 oder neuer
unter `/usr`, wo die Sandbox sie sieht, und wähle es aus:

```sh
sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent
humanitl config set agent.adapter pi
```

pi ist ein Node.js-Skript; ein pi oder ein Node.js unter deinem
Home-Verzeichnis (der Installer von `pi.dev`, nvm) ist in der Sandbox nicht
sichtbar. Siehe [pi verwenden](agent-profiles.md#using-pi).

## Die Installation prüfen

Drei Befehle, in dieser Reihenfolge:

```sh
humanitl daemon status
humanitl doctor
humanitl sandbox check
```

`humanitl daemon status` fragt den Daemon, wer er ist, und endet mit 0:

```
FIELD         VALUE
socket        /run/user/1000/humanitl/daemon.sock
unit          active
daemon        0.1.0
proto         1.0
session       7b1c…
capabilities  hold, rules, sandbox
```

Exit-Code 2 mit `DAEMON_001` heißt, dass auf dem Socket nichts antwortet; siehe
[Fehlerbehebung](#troubleshooting).

`humanitl doctor` gibt elf Zeilen aus (`bwrap`, `userns`, `seccomp`,
`runtime_dir`, `systemd_user`, `daemon`, `agent`, `llm`, `tray`, `renderer`,
`disk_space`). Die ersten vier sind die Voraussetzungen der Sandbox; solange
eine davon fehlschlägt, startet nichts. Die Zeile `llm` sagt „nicht
angesprochen", bis du einen Modellserver einstellst: Der Doctor öffnet keine
Netzwerkverbindung, außer du verlangst es mit `--probe-llm`.

`humanitl sandbox check` startet eine kurzlebige Sandbox, misst darin die drei
Isolationsgarantien und hält sie wieder an:

```
   CHECK                 EVIDENCE
✓  no_network_interface  …
✓  single_socket         …
✓  seccomp_active        …
```

Alle drei müssen `✓` sein. Eine verletzte Garantie beendet den Befehl mit
Exit-Code 3 und dem Befund der ersten Prüfung, die fehlschlug. Was jede Garantie
bedeutet, steht in [SECURITY.md](../SECURITY.md).

## Fehlerbehebung

**`DAEMON_001`, nichts antwortet.** Der Befund nennt einen Befehl, der den
Dienst einrichtet oder startet: `humanitl daemon install`, wenn es noch keine
Unit gibt, `systemctl --user start humanitld.socket` beim Paket und
`systemctl --user start humanitld.service` bei der Unit, die `daemon install`
geschrieben hat. `humanitl daemon logs` zeigt das Journal des Dienstes
(`journalctl --user -u humanitld`). Nur zur Fehlersuche läuft `humanitld` auch im
Vordergrund eines Terminals und endet mit ihm.

**Kein `XDG_RUNTIME_DIR`.** Ohne logind gibt es womöglich kein
`/run/user/<uid>`. Daemon, Kommandozeile und App weichen dann auf ein
Verzeichnis unter `$TMPDIR` aus, das ein anderes Konto zuerst anlegen könnte,
und Humanitl weigert sich, eines zu benutzen, das ihm nicht gehört
(`DAEMON_001`, `DAEMON_004`). Die Abhilfe ist ein eigenes Laufzeitverzeichnis
unter deinem Home-Verzeichnis, gesetzt für die ganze Anmeldung:
[XDG_RUNTIME_DIR ohne logind](../INSTALL.md#xdg_runtime_dir-without-logind).

**`DAEMON_005` bei `daemon install`.** Es gibt eine Datei
`~/.config/systemd/user/humanitld.service`, die `daemon install` nicht
geschrieben hat. Humanitl überschreibt sie nie. Schieb sie beiseite oder mach
aus deinen Änderungen ein Drop-in mit `systemctl --user edit humanitld.service`.

**Schwarzes Fenster beim Start.** Ein geladenes NVIDIA-Modul unter Wayland ist
eine bekannte Ursache; `humanitl doctor` meldet es als `DOCTOR_010`. Der
Release-Build hat noch keinen Schalter auf einen anderen Renderer.

Jeder Code ist in [DIAGNOSTICS.md](../DIAGNOSTICS.md) erklärt.

## Update

- **Paket:** Installiere das neuere `.deb` mit `sudo apt install ./…deb` (oder
  `apt upgrade` aus dem APT-Repository). Sonst gibt es nichts zu tippen: Das
  Paket lädt deine Benutzer-Units neu und startet deinen Daemon mit der neuen
  Version neu. Läuft eine Agenten-Session, läuft der Daemon weiter, bis sie
  endet; ein Daemon jedes Release nach 0.0.6 beendet sich dann von selbst, und
  die nächste Verbindung startet die neue Version. Bis dahin nennt die
  Statusleiste der App die andere Version und bietet **Daemon neu starten** an,
  sobald keine Session läuft; auf der Kommandozeile entspricht dem
  `systemctl --user daemon-reload && systemctl --user restart humanitld`.
- **RPM:** `sudo dnf install ./humanitl-<newer>-1.x86_64.rpm` (openSUSE:
  `sudo zypper install --allow-unsigned-rpm ./…rpm`). Das Scriptlet lädt neu und
  startet neu wie beim `.deb`.
- **AUR:** `git pull` im Checkout von `humanitl-bin`, dann `makepkg -si`, oder
  das Update deines AUR-Helfers. Das Installationsskript lädt neu und startet neu
  wie beim `.deb`.
- **AppImage:** Starte das neuere Image. Es frischt den Dienst von selbst auf.
- **Archiv:** Entpacke die neue Version und führe ihr
  `bin/humanitl daemon install` aus.

## Deinstallieren

Entferne zuerst den Zugangsschlüssel des Modellservers aus dem Schlüsselbund,
falls du einen gespeichert hast, solange die Kommandozeile noch da ist:

```sh
humanitl llm key remove
```

Dann entfernst du den Dienst und die Programme.

**Paket:**

```sh
humanitl daemon uninstall
sudo apt remove --purge humanitl
```

`humanitl daemon uninstall` schaltet den Socket und den Dienst aus (dasselbe wie
`systemctl --user disable --now humanitld.socket humanitld.service`). Die Units
gehören dem Paket, das sie entfernt.

**RPM** unter Fedora:

```sh
humanitl daemon uninstall
sudo dnf remove humanitl
```

**RPM** unter openSUSE:

```sh
humanitl daemon uninstall
sudo zypper remove humanitl
```

**AUR-Paket** unter Arch Linux:

```sh
humanitl daemon uninstall
sudo pacman -Rns humanitl-bin
```

**AppImage:**

```sh
./Humanitl-<version>-x86_64.AppImage --cli daemon uninstall --purge-binaries
rm Humanitl-<version>-x86_64.AppImage
```

Das schaltet den Dienst aus und entfernt die Unit, ihre Verweise, einen
übrig gebliebenen Socket und alles, was `daemon install` unter
`~/.local/lib/humanitl/` abgelegt hat.

**Archiv:**

```sh
humanitl-<version>-linux-x86_64/bin/humanitl daemon uninstall
rm -r humanitl-<version>-linux-x86_64
```

### Deine Daten

Keiner der Befehle oben rührt deine Daten an. Konfiguration, Regeln,
Aufzeichnung, das Audit-Log und der private Schlüssel der lokalen CA gehören
dir, nicht dem Paket, und bleiben, wo sie sind:

| Pfad | Inhalt |
|---|---|
| `~/.config/humanitl/config.toml` | deine Konfiguration ([CONFIG.md](../CONFIG.md)) |
| `~/.config/humanitl/rules.yaml` | deine gespeicherten Regeln ([Regel-Referenz](../reference/rules.md)) |
| `~/.config/humanitl/profiles/` | deine eigenen Session-Profile und unter `sandbox/` deine eigenen Sandbox-Profile |
| `~/.local/share/humanitl/humanitl.db` | die Aufzeichnung: jede Anfrage und jede Antwort |
| `~/.local/share/humanitl/blobs/` | Bodies von Anfragen und Antworten aus der Aufzeichnung |
| `~/.local/share/humanitl/audit/audit.jsonl` | die Audit-Kette |
| `~/.local/share/humanitl/ca/ca.key`, `ca.crt` | die lokale CA, die der Proxy in der Sandbox benutzt; sie wird nie in die Vertrauensablage des Rechners aufgenommen |
| `~/.local/share/humanitl/keys/` | ein als Datei gespeicherter Zugangsschlüssel, wenn der Schlüsselbund (unter macOS das Keychain) nicht verfügbar war |
| `$XDG_RUNTIME_DIR/humanitl/` | Socket und Token des Daemons; nach der Abmeldung weg |

Die Tabelle geht von den Vorgaben `XDG_CONFIG_HOME=~/.config` und
`XDG_DATA_HOME=~/.local/share` aus. Aufzeichnung und Audit-Log können Bodies von
Anfragen enthalten, also lösch sie mit Absicht, nicht aus Versehen. Um alles zu
entfernen:

```sh
rm -r ~/.config/humanitl ~/.local/share/humanitl
```

Das lässt sich nicht rückgängig machen. Exportiere die Audit-Kette vorher, wenn
du sie später brauchst
(`humanitl audit export --format jsonl --out audit-export.jsonl`).
