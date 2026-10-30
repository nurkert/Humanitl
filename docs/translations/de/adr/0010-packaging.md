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
