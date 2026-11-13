---
source: adr/0010-packaging.md
source-sha256: 4cb2a870e1ffd70ba2dbb5089125ff9dd0a59c3ab836b3507589bccb8fc6c472
lang: de
---
# ADR-0010 · Auslieferung als `.deb` und AppImage, Flatpak später und nur für die Oberfläche

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl besteht aus vier ausführbaren Teilen: dem Daemon `humanitld`, der
Flutter-Oberfläche, dem Kommandozeilenwerkzeug `humanitl` und dem Sandbox-Shim
`humanitl-shim`. Dazu kommen Profile, der Domain-Katalog, die Public Suffix List und die
mitgelieferten Regeln.

Die Zielgruppe sind Fachleute ohne Sicherheitshintergrund. Für sie muss gelten, was
`BACKLOG.md` 1.3 als Grundsatz 9 festhält: Ein Paket installiert alles, der erste Start
bietet den Hintergrunddienst mit einem Klick an, und danach funktioniert der Standardweg
ohne Terminal.

Die technische Randbedingung, die alles andere bestimmt: **Der Daemon startet `bwrap`.**
Ein abgeschottetes Anwendungsformat, das selbst keine verschachtelten User-Namespaces
erlaubt, kann den Daemon nicht ausführen. Das betrifft vor allem Flatpak.

## Entscheidung

Ausgeliefert wird zuerst als **`.deb`** und als **AppImage**, gebaut mit
[fastforge](https://fastforge.dev/). Beide Artefakte enthalten alles: Oberfläche, Daemon,
Shim, Kommandozeile, Profile, Katalog und Regeln. Das Daemon-Binary liegt neben dem
Flutter-Bündel, nicht in einem eigenen Paket.

Der Hintergrunddienst ist eine **systemd-User-Unit**, die beim ersten Start über
`FixAction::InstallService` angeboten und mit einem Klick installiert und gestartet wird.
Kein Terminal nötig. Die Unit ist gehärtet: `NoNewPrivileges`, `ProtectHome=read-only` mit
gezielten `ReadWritePaths`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`,
`PrivateTmp`, `SystemCallFilter=@system-service`.

Das AppImage legt Unit und Binaries unter `~/.local` ab und aktualisiert sie bei einem
Versionswechsel. Die Deinstallation entfernt beides. `humanitl doctor` bestätigt danach,
dass die Installation vollständig ist.

**Flatpak kommt später und nur für die Oberfläche.** Der Daemon läuft dann außerhalb des
Flatpaks, und die Verbindung entsteht über `--filesystem=xdg-run/humanitl`, also über den
gRPC-Socket aus ADR-0003.

## Begründung

`.deb` ist das Format der wichtigsten Zielplattform (Debian, Ubuntu und Ableger). Es fügt
sich in die Paketverwaltung ein, kann Abhängigkeiten wie `bubblewrap` erklären und ist
das, was die Zielgruppe erwartet.

AppImage deckt alles daneben ab, ohne eine Matrix von Paketformaten. Es braucht keine
Root-Rechte und ist das kürzeste „probier es einmal aus" für ein Testpublikum vor dem
Release.

Zusammen decken beide die realistische Landschaft der Linux-Desktops ab, ohne dass wir für
jede Distribution ein eigenes Rezept pflegen.

Dass Daemon und Oberfläche in **einem** Artefakt liegen, ist eine Entscheidung gegen die
übliche Aufteilung in `humanitl` und `humanitl-daemon`. Der Grund ist Grundsatz 9: Ein
Paket, das die Hälfte des Produkts installiert und dann sagt „bitte installiere auch noch
das Daemon-Paket", ist genau die Reibung, die vermieden werden soll. Außerdem müssen die
Versionen von Daemon und Proto zusammenpassen (ADR-0003); ein einziges Artefakt macht eine
Versionsabweichung unmöglich.

Die systemd-User-Unit statt eines Systemdienstes: Humanitl braucht keine Root-Rechte,
arbeitet auf `$XDG_RUNTIME_DIR` und `$XDG_DATA_HOME` des Nutzers und startet mit `bwrap`
eine Sandbox ohne Root. Ein Systemdienst wäre mehr Berechtigung ohne Gewinn und würde
Mehrbenutzerfragen aufwerfen, die es nicht gibt.

Flatpak für die Oberfläche ist attraktiv (Verteilung über Flathub, automatische Updates),
scheitert aber am Daemon: Eine Flatpak-Sandbox kann `bwrap` nicht so starten, wie wir es
brauchen. Die Aufteilung „Oberfläche im Flatpak, Daemon außerhalb" ist der einzige
gangbare Weg, und er funktioniert nur, weil der Daemon ohnehin ein eigener Prozess mit
einer sauberen Socket-Schnittstelle ist (ADR-0003). Dafür kostet er einen zusätzlichen
Installationsschritt für den Daemon und kommt deshalb zuletzt.

## Verworfene Alternativen

- **Nur Flatpak.** Die eleganteste Verteilung und für dieses Produkt technisch unmöglich,
  solange der Daemon `bwrap` startet.
- **Snap.** Ähnliche Sandbox-Probleme, dazu ein Store, den ein Teil der Zielgruppe meidet,
  und ein schwierigerer Umgang mit `$XDG_RUNTIME_DIR`.
- **Nur ein AppImage.** Spart das `.deb`, verliert aber die Erklärung von
  Systemabhängigkeiten (`bubblewrap`) und die vertraute Installation über die
  Paketverwaltung.
- **Getrennte Pakete für Daemon und Oberfläche.** Aus Sicht eines Paketierers sauberer,
  aber ein Bruch mit Grundsatz 9 und ein Risiko für Versionsabweichungen zwischen
  Proto-Client und Server.
- **Systemdienst statt User-Unit.** Mehr Berechtigung, kein Gewinn, und Root-Rechte bei
  der Installation, die ein Aufbau ohne Root nicht braucht.
- **`cargo install` und `flutter build` als Installationsanleitung.** Gut für Entwickler,
  keine Auslieferung für die Zielgruppe.
- **Von anderen gepflegte Distributionspakete (AUR, Fedora COPR).** Willkommen, aber kein
  Ersatz für ein Artefakt, das wir selbst bauen und testen.

## Folgen

- Der Release-Job baut beide Artefakte, erzeugt Prüfsummen und hängt sie an den Tag. Die
  Demo-Skripte der Meilensteine M1 bis M4 müssen dafür grün sein.
- Die systemd-Unit gehört zum Repository (`packaging/systemd/`) und wird mitgetestet,
  nicht bei der Auslieferung improvisiert.
- Der erste Start braucht einen Weg ohne Terminal. Das ist der Grund, warum
  `FixAction::InstallService` als aufzählbare Aktion existiert (ADR-0012) und nicht als
  Textanleitung.
- Wayland ist der Normalfall, X11 die Rückfallebene. Beide werden vor dem Release auf
  NVIDIA- und Intel-Grafik getestet, weil Flutter auf dem Desktop dort anders aussieht.
- Ein Update muss Datenbankmigrationen (ADR-0008) und Proto-Versionen (ADR-0003)
  berücksichtigen. Das Paket bringt beide Seiten zugleich mit, was den Fall „Oberfläche
  neuer als Daemon" auf den Sonderfall Flatpak beschränkt; dort greift die
  Versionsprüfung aus `GetInfo`.
- Die Deinstallation muss Unit und Binaries wieder entfernen, sonst bleibt ein Dienst
  zurück, der auf einen fehlenden Socket zeigt.

## Betroffene Issues

`HUM-053` (Paketierung: `.deb` und AppImage über fastforge, systemd-User-Unit,
Wayland-Test), `HUM-077` (Installation und Deinstallation mit einem Klick), `HUM-075`
(`humanitl doctor` bestätigt die Installation), `HUM-060` (Release 0.1 mit Changelog und
Prüfsummen).

## Nachtrag 2026-11-13: Podman unter der gehärteten Nutzer-Unit (HUM-434)

Der übrige Nachtrag zum Podman-Backend folgt mit HUM-441; dieser Teil betrifft die Unit.

**Gemessen.** Unter `humanitld.service` startet rootless Podman nicht als Kind des Daemons.
Messung P8 (HUM-292, 2026-11-13, Podman 5.8.6, vorübergehende Nutzer-Units aus der Unit-Datei,
je eine Eigenschaft weggelassen): Jede Eigenschaft mit eigenem Mount-Namensraum bricht
`newuidmap` („write to uid_map failed"): `PrivateTmp`, `ProtectSystem`, `PrivateMounts`,
`ProtectClock`, `ProtectControlGroups`, `ProtectKernelModules`, `PrivateIPC` und `PrivateUsers`
(in einer Nutzer-Unit zieht jede einen eigenen Nutzer-Namensraum nach sich).
`CapabilityBoundingSet=CAP_SYS_ADMIN` scheitert beim Start (218). Allein bestehen:
`NoNewPrivileges` (nur mit schon vorhandenem Pausenprozess), `ProtectProc`, `KeyringMode`,
`LockPersonality`, `RestrictRealtime`, `RestrictFileSystems`, `SystemCallFilter`,
`SystemCallErrorNumber`, `RestrictAddressFamilies`. Eine vorübergehende Unit ohne die
Eigenschaften mit Mount-Namensraum startet `podman run --rootfs`.

**Entscheidung (Inhaber, 2026-11-13): Weg (b).** Die Unit zu lockern (Weg a) hätte acht
Eigenschaften gekostet, den Großteil der Härtung, für jedes Backend. Stattdessen bleibt die Unit
genau, wie sie ist, und Podman startet über den Dienstverwalter des Nutzers:

- Läuft der Daemon außerhalb des ersten Nutzer-Namensraums (`/proc/self/uid_map` ist nicht
  `0 0 4294967295`, so unter der Unit), läuft jeder Aufruf von Podman, die Sitzung wie jede
  Frage, als vorübergehende Unit über `systemd-run --user --quiet --collect --pipe --wait
  --service-type=exec --expand-environment=no`, `KillMode=mixed`, `Delegate=yes`, `TimeoutStopSec` gleich der
  Frist zum Beenden (5 s), mit dem Namen `humanitl-<sitzung>`; eine Frage heißt
  `humanitl-q-<id>` und hat `RuntimeMaxSec` knapp über ihrer Frist. Das Programm läuft hinter
  `/usr/bin/env -i` mit genau der Umgebung, die der Daemon ihm gibt. Gebaut werden nur Namen mit
  dem Präfix `humanitl-`, jedes Argument muss UTF-8 sein, und das Programm ein absoluter Pfad
  ohne `=`. Aus einem Terminal, in der CI und in den Tests startet Podman weiter als Kind.
- `systemd-run` und nicht `StartTransientUnit` über D-Bus: Im Workspace gibt es keine
  D-Bus-Crate, und `systemd-run` kann schon, was gebraucht wird (Standardströme über `--pipe`,
  den Exit-Status über `--wait`, das Wegräumen über `--collect`). Für `--expand-environment=no`
  braucht es systemd 254 oder neuer; ein älterer oder nicht erreichbarer Dienstverwalter ist
  `SANDBOX_050`.
- Die Deskriptoren des Shims reisen nicht mehr über `--preserve-fds`, das nur für ein Kind geht:
  Der Daemon bindet je Sitzung einen `SOCK_SEQPACKET`-Socket (`0600`, im Verzeichnis der Sitzung
  in einem eigenen Verzeichnis mit `0700`), bindet nur dieses Verzeichnis nur lesbar in die
  Sandbox ein (`/run/humanitl-handover`) und nennt Socket und ein Einmal-Zeichen von 128 Bit in
  der Zeile des Shims. Er nimmt genau eine Verbindung an, schließt den lauschenden Socket und
  entfernt die Socket-Datei gleich nach dem `accept`, sodass kein zweiter Socket in der Sandbox
  bleibt (ESC-2), prüft `SO_PEERCRED` gegen die eigene Kennung und das Zeichen
  und schickt dann alle Deskriptoren in einer Nachricht. Der Shim legt sie wie bisher auf 3, 4,
  5, ..., sodass memfd der Umgebung, Mounttabelle und Lebensleine unverändert gelesen werden.
  Stirbt der Daemon vor der Übergabe, findet der Shim niemanden und startet den Agenten nie;
  danach beendet die Lebensleine die Sandbox wie bisher. Derselbe Weg dient auch einem Podman,
  das als Kind startet, damit überall ein Weg geprüft wird.
- Das Beenden einer Sitzung stoppt ihre Unit (`systemctl --user stop --no-block`), nachdem der
  Container beendet und entfernt ist; das Ende eines Clients ohne Stopp (HUM-452) ist das Ende
  von `systemd-run --wait`, also das Ende des Hauptprozesses der Unit. Ein abgebrochenes Ziehen
  wartet, bis seine Unit gestoppt ist, bevor es sagt, dass nichts mehr davon läuft.
- Unter der Unit sieht die Engine die Mount-Flags der Quellen aus dem Namensraum des
  Dienstverwalters, der Daemon aus seinem eigenen, in dem die Unit den Rechner mit `nosuid` neu
  einhängt. Die erwartete Mounttabelle lässt deshalb auf diesem Weg die geerbten Flags der Binds
  offen und bleibt bei Punkt, Typ, Modus, Anzahl sowie Gerät und Inode streng; der direkte Weg
  bleibt ganz streng.
- bubblewrap und Seatbelt bleiben unberührt: Sie erben ihre Deskriptoren wie bisher.

**Preis.** Ein übernommener Daemon kann eine vorübergehende Unit des Nutzers starten, außerhalb
seiner Härtung (`docs/THREAT-MODEL.md`, Vertrauensgrenzen). Das Konto konnte das vorher schon,
der gehärtete Daemon nicht. Die Unit des Daemons selbst gewinnt kein Recht und keinen Pfad.

## Nachtrag 2026-11-13: Podman ist empfohlen, nie verlangt (HUM-441)

Das `.deb` führt `podman` und `uidmap` unter `Recommends`, neben
`libsecret-tools` (`packaging/deb/control.in`), nie unter `Depends`. `uidmap`
bringt `newuidmap` und `newgidmap`, die Podman ohne Root für die
untergeordneten IDs des Kontos braucht; ohne sie endet ein Start unter Podman
mit `SANDBOX_039`, dessen Fix `sudo apt install uidmap` nennt. bubblewrap
bleibt das einzige Sandbox-Programm unter `Depends`, und
`sandbox.backend = "auto"` bleibt bubblewrap, also installiert und läuft das
Paket ohne Podman. `apt install` zieht empfohlene Pakete als Vorgabe mit, und
`--no-install-recommends` lässt sie weg. `packaging/deb/check-install.sh`
(Release-Workflow) und `check-upgrade.sh` (CI) installieren genau so, dort
also ohne Podman, und `check-upgrade.sh` startet danach seinen Daemon. Fehlt Podman oder ist es zu alt (unter 4.9.0),
ist das ein Befund von `humanitl sandbox backends` und eines Starts, der es
gewählt hat (`SANDBOX_034`, `SANDBOX_037`), nie ein Grund, das Paket
abzulehnen oder auf ein anderes Backend auszuweichen.

Eine Laufzeit für die Kernel-Stufe steht nicht in der Liste: `krun` ist nicht
zugelassen (HUM-449), und Debian paketiert libkrun nicht. Die Unit bleibt, wie
der Nachtrag oben sie festlegt (HUM-434): Podman startet als vorübergehende
Unit des Dienstverwalters des Nutzers, was systemd 254 oder neuer braucht, und
die Unit des Daemons behält ihre volle Härtung. Das RPM und das AUR-Paket
nennen Podman noch nicht.
