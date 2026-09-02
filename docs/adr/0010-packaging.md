# ADR-0010 · Auslieferung als `.deb` und AppImage, Flatpak später und nur für die UI
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl besteht aus vier ausführbaren Teilen: dem Daemon `humanitld`, der
Flutter-Oberfläche, dem Kommandozeilenwerkzeug `humanitl` und dem
Sandbox-Shim `humanitl-shim`. Dazu kommen Profile, der Domain-Katalog, die
Public Suffix List und die mitgelieferten Regeln.

Die Zielgruppe sind Professionelle ohne Security-Hintergrund. Für sie muss
gelten, was `BACKLOG.md` 1.3 als Prinzip 9 formuliert: Ein Paket installiert
alles, der Erststart bietet den Hintergrunddienst mit einem Klick an, und
danach funktioniert der Standardweg ohne Terminal.

Die technische Randbedingung, die alles andere bestimmt: **Der Daemon startet
`bwrap`.** Ein sandboxed Anwendungsformat, das selbst keine User-Namespaces
verschachteln lässt, kann den Daemon nicht ausführen. Das betrifft insbesondere
Flatpak.

## Entscheidung

Ausgeliefert wird zuerst als **`.deb`** und als **AppImage**, gebaut mit
[fastforge](https://fastforge.dev/). Beide Artefakte enthalten alles: UI, Daemon,
Shim, CLI, Profile, Katalog und Regeln. Das Daemon-Binary liegt neben dem
Flutter-Bundle, nicht in einem separaten Paket.

Der Hintergrunddienst ist eine **systemd user unit**, die beim Erststart über
`FixAction::InstallService` angeboten und mit einem Klick installiert und
gestartet wird. Kein Terminal erforderlich. Die Unit ist gehärtet:
`NoNewPrivileges`, `ProtectHome=read-only` mit gezielten `ReadWritePaths`,
`RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, `PrivateTmp`,
`SystemCallFilter=@system-service`.

Das AppImage legt Unit und Binaries unter `~/.local` an und aktualisiert sie bei
einem Versionswechsel. Die Deinstallation entfernt beides. `humanitl doctor`
bestätigt anschließend, dass die Installation vollständig ist.

**Flatpak kommt später und nur für die UI.** Der Daemon läuft dann außerhalb des
Flatpak, die Verbindung entsteht über `--filesystem=xdg-run/humanitl`, also über
den gRPC-Socket aus ADR-0003.

## Begründung

`.deb` ist das Format der primären Zielplattform (Debian, Ubuntu und Ableger).
Es integriert sich in die Paketverwaltung, kann Abhängigkeiten wie `bubblewrap`
deklarieren und wird von der Zielgruppe erwartet.

AppImage deckt alles daneben ab, ohne Paketformat-Matrix. Es braucht keine
Root-Rechte und ist für ein Vorabtest-Publikum das kürzeste „einmal ausprobieren".

Beide zusammen decken die realistische Linux-Desktop-Landschaft ab, ohne dass wir
für jede Distribution ein eigenes Rezept pflegen.

Dass Daemon und UI in **einem** Artefakt liegen, ist eine Entscheidung gegen die
übliche Aufteilung in `humanitl` und `humanitl-daemon`. Der Grund ist Prinzip 9:
Ein Paket, das die Hälfte des Produkts installiert und dann sagt „installieren
Sie noch das Daemon-Paket", ist genau die Reibung, die vermieden werden soll.
Zudem müssen Daemon- und Proto-Version zusammenpassen (ADR-0003); ein einziges
Artefakt macht eine Versionsabweichung unmöglich.

Die systemd user unit statt eines System-Dienstes: Humanitl braucht keine
Root-Rechte, arbeitet auf `$XDG_RUNTIME_DIR` und `$XDG_DATA_HOME` des Nutzers und
startet mit `bwrap` eine rootless Sandbox. Ein System-Dienst wäre mehr
Berechtigung ohne Gewinn und würde Mehrbenutzerfragen aufwerfen, die es nicht
gibt.

Flatpak für die UI ist attraktiv (Distribution über Flathub, automatische
Updates), scheitert aber am Daemon: Ein Flatpak-Sandbox kann `bwrap` nicht so
starten, wie wir es brauchen. Die Aufteilung „UI im Flatpak, Daemon außerhalb"
ist der einzige gangbare Weg — und sie funktioniert nur deshalb, weil der Daemon
ohnehin ein eigener Prozess mit einer sauberen Socket-Schnittstelle ist
(ADR-0003). Sie kostet dafür einen zusätzlichen Installationsschritt für den
Daemon und steht deshalb hinten.

## Verworfene Alternativen

- **Nur Flatpak.** Die eleganteste Distribution und für dieses Produkt technisch
  unmöglich, solange der Daemon `bwrap` startet.
- **Snap.** Ähnliche Sandbox-Probleme, dazu ein Store, den ein Teil der
  Zielgruppe meidet, und eine schwierigere Handhabung von `$XDG_RUNTIME_DIR`.
- **Nur ein AppImage.** Spart das `.deb`, verliert aber die Deklaration von
  Systemabhängigkeiten (`bubblewrap`) und die vertraute Installation über die
  Paketverwaltung.
- **Getrennte Pakete für Daemon und UI.** Sauberer aus Paketierersicht, aber ein
  Bruch mit Prinzip 9 und ein Risiko für Versionsabweichungen zwischen
  Proto-Client und -Server.
- **System-Dienst statt user unit.** Mehr Berechtigung, kein Gewinn, und
  Root-Rechte bei der Installation, die für einen rootless Aufbau nicht nötig
  sind.
- **`cargo install` und `flutter build` als Installationsanleitung.** Für
  Entwickler in Ordnung, für die Zielgruppe keine Auslieferung.
- **Distributionspakete in Fremdverantwortung (AUR, Fedora COPR).** Willkommen,
  aber kein Ersatz für ein Artefakt, das wir selbst bauen und testen.

## Konsequenzen

- Der Release-Job baut beide Artefakte, erzeugt Prüfsummen und hängt sie an das
  Tag. Die Demo-Skripte der Milestones M1 bis M4 müssen dafür grün sein.
- Die systemd-Unit ist Teil des Repositories (`packaging/systemd/`) und wird
  mitgetestet, nicht bei der Auslieferung improvisiert.
- Der Erststart braucht einen Pfad ohne Terminal. Das ist der Grund, warum
  `FixAction::InstallService` als aufzählbare Aktion existiert (ADR-0012) und
  nicht als Textanleitung.
- Wayland ist der Normalfall, X11 der Rückfall. Beides wird vor dem Release auf
  NVIDIA- und Intel-Grafik getestet, weil Flutter-Desktop dort unterschiedlich
  aussieht.
- Ein Update muss Datenbank-Migrationen (ADR-0008) und Proto-Versionen
  (ADR-0003) berücksichtigen. Das Paket bringt beide Seiten gleichzeitig mit, was
  den Fall „UI neuer als Daemon" auf den Sonderfall Flatpak beschränkt — dort
  greift der Versionscheck aus `GetInfo`.
- Die Deinstallation muss Unit und Binaries wieder entfernen, sonst bleibt ein
  Dienst zurück, der auf einen fehlenden Socket zeigt.

## Betroffene Issues

`HUM-053` (Packaging: `.deb` und AppImage via fastforge, systemd user unit,
Wayland-Test), `HUM-077` (Ein-Klick-Installation und Deinstallation),
`HUM-075` (`humanitl doctor` bestätigt die Installation), `HUM-060`
(Release 0.1 mit Changelog und Prüfsummen).
