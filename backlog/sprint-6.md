# Sprint 6 · macOS (M12)

Ziel des Sprints: Humanitl läuft auf macOS (Apple Silicon und Intel) so sauber wie auf Linux, und jedes Release trägt eine funktionierende Mac-App. Wunsch des Nutzers vom 2026-10-29. Entscheidungen des Nutzers vom selben Tag: Die Sandbox auf macOS ist zuerst Seatbelt (`sandbox-exec` mit einem erzeugten SBPL-Profil), ein Backend mit Linux-VM folgt später als eigenes Issue (HUM-328). Die App wird ohne Apple Developer ID nur ad-hoc signiert. Die Arbeit landet auf dem Feature-Branch `macos-support`, ein Commit je Issue, und geht erst nach `main`, wenn Linux und macOS grün sind; das weicht bewusst von `CLAUDE.md` Schritt 5 ab.

Nummern: Die Issues HUM-323 bis HUM-332 hießen auf dem Branch zuerst HUM-313 bis HUM-322. HUM-313 bis HUM-316 hat `main` parallel an Issues aus Sprint 5 vergeben, deshalb ist der ganze Block um zehn verschoben; die Commit-Texte des Branches tragen noch die alten Nummern.

Leitlinie: Linux bleibt, wie es ist. Jede Änderung an gemeinsamem Code ist entweder plattformneutral oder hinter `#[cfg(target_os = ...)]` beziehungsweise `Platform.isMacOS`, und die Linux-CI bleibt die Referenz.

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-312 | Daemon, CLI und Tests bauen auf macOS | M | — |
| HUM-323 | Seatbelt-Backend: die Sandbox auf macOS | L | HUM-312 |
| HUM-324 | Der Daemon läuft auf macOS als launchd-Agent | M | HUM-312 |
| HUM-325 | Die App läuft auf macOS | M | HUM-312 |
| HUM-326 | Jedes Release trägt eine Mac-App | M | HUM-323, HUM-324, HUM-325 |
| HUM-327 | Sicherheitsaussage und Bedrohungsmodell für macOS | S | HUM-323 |
| HUM-328 | Linux-VM als zweites Backend auf macOS (später) | L | HUM-323 |
| HUM-329 | Tray und Benachrichtigungen auf macOS (später) | M | HUM-325 |
| HUM-330 | Vorschau und Mounttabelle zeigen das Seatbelt-Profil | S | HUM-323 |
| HUM-331 | Der Proxy nimmt unter macOS nur den Shim der laufenden Sitzung an | M | HUM-323 |
| HUM-332 | Kleinere Folgen aus den Reviews von M12 | S | HUM-312, HUM-323 bis HUM-327 |
| HUM-333 | Die Website humanitl.de | M | HUM-326 |
| HUM-334 | Die Bühne von humanitl.de: Glas, Licht, Einflug und Morph | M | HUM-333 |
| HUM-335 | Linux: das AppImage in jedem Release, APT und AppImage auf der Download-Seite | S | HUM-333 |
| HUM-336 | Die Dokumentation aus docs/ auf humanitl.de | M | HUM-333 |
| HUM-345 | Die Sicherheitsseite von humanitl.de | M | HUM-333, HUM-336 |
| HUM-344 | Eine README, die das Projekt zeigt | M | HUM-333 |
| HUM-342 | Die Ampel unter macOS sitzt mittig in der Titelleiste der App | S | HUM-325 |

## HUM-312 · Daemon, CLI und Tests bauen auf macOS

### Kontext
`cargo check --workspace` scheitert auf macOS am 2026-10-29 an drei Stellen und verdeckt dahinter weitere: `seccompiler` gibt es nur für Linux, `rustix::fs::Mode::from_raw_mode` nimmt auf macOS `u16`, `libc::stat::st_dev` ist dort `i32`. Dazu kommen Linux-Aufrufe ohne Gegenstück (`memfd_create`, `openat2`, `SOCK_SEQPACKET`, abstrakte Unix-Sockets, `/proc`). Ein Laufzeitfehler ohne Compilerfehler: `Env::uid()` liest die UID aus `metadata("/proc/self")` und liefert auf macOS 0; die CLI verweigert dann das Token mit DAEMON_001.

### Spezifikation
1. Der Shim (`humanitl-shim`) und alles, was seccomp, Namespaces oder bwrap braucht, steht hinter `cfg(target_os = "linux")`. Abhängigkeiten, die es nur unter Linux gibt (`seccompiler`), stehen unter `[target.'cfg(target_os = "linux")'.dependencies]`.
2. Die UID kommt aus `getuid()`, nie aus `/proc`.
3. Die Grenze für `sun_path` kommt aus `sockaddr_un` (108 auf Linux, 104 auf macOS), nicht aus einer festen Zahl.
4. `cargo build --workspace --all-targets` und `cargo test --workspace` sind auf macOS grün; Tests, die Linux voraussetzen, stehen hinter `cfg`.
5. CI-Job `macos` auf `macos-15` (arm64): Build, Test, Clippy des Workspace.

### Akzeptanzkriterien
1. `cargo clippy --workspace --all-targets -- -D warnings` grün auf macOS und Linux.
2. Ein Test belegt `Env::uid() == getuid()`.
3. `tools/check_coupling.py` bleibt grün.

## HUM-323 · Seatbelt-Backend: die Sandbox auf macOS

### Kontext
Auf macOS gibt es keine Namespaces, kein bwrap und kein seccomp. Das Gegenstück ist Seatbelt: `sandbox-exec -f profil.sb` setzt ein SBPL-Profil, das der Kern für den Prozess und alle Kinder durchsetzt; Chrome, Codex und Claude Code `sandbox-runtime` nutzen es. Am 2026-10-29 auf macOS 27.0.1 gemessen, mit `(deny default)`, Tür-Socket und einem Loopback-Port erlaubt: Die Tür antwortet; TCP nach außen, UDP-Senden, DNS, Raw-Sockets, IPv6, `AF_ROUTE`, `AF_SYSTEM`, `AF_NDRV`, fremde Unix-Sockets und Schreiben außerhalb des Projekts enden mit `EPERM`. `socket()` selbst wird für `AF_INET`, `AF_INET6` und `AF_UNIX` nicht vermittelt, aber jedes `bind`, `connect` und `sendto` geht über `network-*`. Bei SBPL gewinnt die letzte passende Regel.

Seatbelt kennt keine Mounts. Der Agent sieht echte Pfade, nicht `/work` oder `/home/agent`.

### Spezifikation
1. `SeatbeltBackend` implementiert `SandboxBackend` mit denselben Builder-Methoden wie `BwrapBackend`. Der Daemon und die CLI sprechen die Sandbox über einen Plattformtyp an (`PlatformBackend`), nicht über `BwrapBackend`.
2. **Sitzungsverzeichnis.** Je Sitzung ein privates Verzeichnis (0700) unter dem Laufzeitverzeichnis mit `home`, `tmp`, `etc`, `run`. Die Sandbox-Pfade des Plans werden darauf abgebildet: `/work` auf den Projektordner, `/home/agent` auf `home`, `/tmp` und `/var/tmp` auf `tmp`, `/etc/humanitl` auf `etc/humanitl`, `/run/humanitl` auf `run`, das CA-Bündel auf `etc/ssl/ca-bundle.crt`. Die Abbildung gilt für Umgebung, Kommando und den Text der Adapter-Dateien, an Pfadgrenzen. Ziele, die sich nicht abbilden lassen (`/etc/opencode/opencode.json`), entfallen mit einer Warnung im Plan.
3. **Profil.** `(deny default)`. Lesen: Systempfade (`/System`, `/usr`, `/bin`, `/sbin`, `/Library`, `/opt/homebrew`, `/usr/local`, `/private/etc`, `/dev`, Werkzeuge von Xcode) und alles außerhalb von `/Users`, `/Volumes` und `/private/var/folders`; aus diesen nur Projekt, Sitzungsverzeichnis, `extra_ro` und das Agent-Programm als einzelne Datei. Schreiben: Projekt (bei `rw`), Sitzungsverzeichnis, `extra_rw`, `/dev/null`, `/dev/tty*`. Die Masken des Profils (`masked_files`, die `tmpfs` unter `/work`) werden zu `deny file-read*` und `deny file-write*` auf genau diesen Pfaden. Netz: `network-outbound` nur zum Tür-Socket und zum Loopback-Port der Sitzung, `network-bind` und `network-inbound` nur auf diesem Port.
4. **Tür.** Der Shim läuft auch auf macOS, ohne seccomp: Er bindet `127.0.0.1:<port>` und reicht jede Verbindung an den Proxy-Socket weiter. Weil Loopback auf macOS mit dem Host geteilt ist, wählt der Daemon je Sitzung einen freien Port; der Shim scheitert geschlossen, wenn er belegt ist.
5. **Garantien auf macOS**, als CHECK-Zeilen des Shims und im `isolation_check`: G1 kein Weg nach außen (Verbindung zu einer öffentlichen IPv4- und IPv6-Adresse und UDP-Senden enden mit `EPERM`), G2 genau eine Tür (Tür erreichbar, ein fremder Unix-Socket des Systems und ein fremder Loopback-Port nicht), G3 Profil aktiv (`sandbox_check` meldet den eigenen Prozess als eingeschränkt).
6. Terminal über PTY wie unter Linux; eine weitere Shell (`sandbox exec`) startet als neuer Prozess unter demselben Profil.
7. `humanitl sandbox check` und eine Escape-Suite `tests/escape/macos` laufen im CI-Job `macos`.

### Akzeptanzkriterien
1. `humanitl sandbox check` auf macOS meldet G1, G2, G3 bestanden; ein Profil ohne Netzsperre lässt G1 scheitern (Mutationsprobe).
2. Aus der Sandbox: `curl https://example.com` ohne Proxy scheitert, mit Proxy wird die Anfrage gehalten; `cat ~/.ssh/*` scheitert; Schreiben in `/work/.git/hooks` scheitert.
3. Linux unverändert: `tests/escape/` grün.

## HUM-324 · Der Daemon läuft auf macOS als launchd-Agent

### Spezifikation
1. Laufzeitverzeichnis auf macOS: `confstr(_CS_DARWIN_USER_TEMP_DIR)/humanitl`, für Terminal und launchd dieselbe Stelle und kurz genug für `sun_path`.
2. `humanitl daemon install|uninstall|restart|status|logs` auf macOS über eine LaunchAgent-Datei `~/Library/LaunchAgents/dev.humanitl.humanitld.plist` (`RunAtLoad`, `KeepAlive` bei Fehlschlag) und `launchctl bootstrap|bootout|kickstart|print`; Protokoll nach `~/Library/Logs/Humanitl/humanitld.log`.
3. Diagnosen nennen auf macOS `launchctl` statt `systemctl`.
4. `humanitl doctor` prüft auf macOS `sandbox-exec`, das Laufzeitverzeichnis, den LaunchAgent und lässt bwrap, user namespaces, seccomp, tray und renderer weg.

## HUM-325 · Die App läuft auf macOS

### Spezifikation
1. `app/macos` mit Titel, Größe 1280×720, ohne doppelte Titelleiste, App Sandbox aus (die App muss Socket, Token und die CLI neben sich erreichen).
2. Die Prüfung privater Pfade nutzt auf macOS `lstat` statt `statx`.
3. Fensterport auf macOS aktiv; die eigenen Fensterknöpfe entfallen dort zugunsten der Ampel.
4. Tastenkürzel nehmen auf macOS Cmd statt Ctrl.
5. Neustart des Daemons und Einrichtung gehen über die CLI (`humanitl daemon restart|install`), nicht über `systemctl`. Die CLI liegt in `Humanitl.app/Contents/MacOS/bin`.
6. Dateien öffnen mit `open`; Zustandsdateien unter `~/Library/Application Support/Humanitl`.

## HUM-326 · Jedes Release trägt eine Mac-App

### Spezifikation
1. Der Release-Workflow baut auf `macos-15` die Binaries für `aarch64-apple-darwin` und `x86_64-apple-darwin`, verbindet sie mit `lipo`, baut die App, legt `humanitld`, `humanitl` und `humanitl-shim` in `Contents/MacOS/bin`, signiert ad-hoc (`codesign --force --deep -s -`) und packt `Humanitl-<version>-macos.dmg` und `.zip`.
2. Ein Prüfjob auf einem frischen macOS-Runner entpackt das Archiv, prüft Signatur und Architekturen, startet den Daemon aus dem Bündel und fährt `humanitl sandbox check`.
3. Das Release führt beide Dateien und ihre Prüfsummen in `SHA256SUMS`; die Release-Notes nennen den Quarantäne-Schritt.

## HUM-327 · Sicherheitsaussage und Bedrohungsmodell für macOS

### Spezifikation
`docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen die Garantien je Backend, die Abweichungen von Linux (Host-geteilter Loopback, kein PID-Namensraum, Leserechte nach Pfaden statt Mounts, Seatbelt ist undokumentiert und als veraltet markiert) und ihre Folgen. ADR-0019 hält die Wahl von Seatbelt fest. `README.md` und `docs/INSTALL.md` nennen macOS.

## HUM-328 · Linux-VM als zweites Backend auf macOS (später)

Entscheidung des Nutzers vom 2026-10-29: nach Seatbelt. Eine Linux-VM über `Virtualization.framework` mit dem bestehenden bwrap-Backend im Gast, die Tür als vsock-Port. Damit gelten auf macOS dieselben drei Garantien wie auf Linux. Umfang, Gastabbild und Startzeit legt ein Spike fest.

## HUM-329 · Tray und Benachrichtigungen auf macOS (später)

Unter Linux laufen Tray und Benachrichtigungen über D-Bus (StatusNotifierItem, `org.freedesktop.Notifications`). Auf macOS braucht es ein `NSStatusItem` und `UNUserNotificationCenter` mit den Aktionen Erlauben, Blockieren, Anzeigen. Bis dahin zeigt die App auf macOS den Hinweis „kein Tray“ wie auf einem Linux ohne Tray.

## HUM-330 · Vorschau und Mounttabelle zeigen das Seatbelt-Profil

### Kontext
Aus HUM-323: Der Plan-RPC baut seine Vorschau (`argv_preview`, Mounttabelle der Oberfläche, `humanitl sandbox argv`) aus `SandboxProfile::to_bwrap_args` und liest die Mounts aus deren Flags (`ipc/src/sandbox.rs`, `Prepared::argv`, `mounts_of`). Unter macOS startet aber `sandbox-exec` mit einem SBPL-Profil; die Vorschau zeigt dort eine bwrap-Zeile, die nie läuft.

### Spezifikation
1. Unter macOS zeigt die Vorschau die Kommandozeile von `sandbox-exec` und den Text des Profils, ohne Seiteneffekt (kein Sitzungsverzeichnis, kein Port).
2. Die Mounttabelle kommt aus derselben Beschreibung wie das Profil (`SeatbeltPolicy`): Projekt mit Modus, Sitzungsverzeichnis, zusätzliche Ordner, Masken.
3. Der Schalter in der Oberfläche heißt neutral („Exakte Sandbox-Kommandozeile").

## HUM-331 · Der Proxy nimmt unter macOS nur den Shim der laufenden Sitzung an

### Kontext
Aus der Review von HUM-327 (K-19, K-20): Unter macOS fehlen PID-Namensraum und seccomp. Ein Prozess, der sich mit `setsid` aus der Sitzung des Agenten löst, überlebt das Ende der Sitzung, und das Seatbelt-Profil erlaubt ihm weiter den Proxy-Socket des Daemons, der für den ganzen Lauf des Daemons gilt. Er kann Anfragen stellen, solange der Daemon läuft; `allow`-Regeln und der Passthrough gelten. Dazu erreicht jeder lokale Prozess jedes Kontos den Loopback-Port der Tür.

### Spezifikation
1. Der Proxy liest unter macOS bei jeder Verbindung auf dem Proxy-Socket die PID der Gegenseite (`getsockopt(LOCAL_PEERPID)`) und nimmt nur Verbindungen der Shims laufender Sitzungen an; der Daemon trägt die PID beim Start ein und beim Ende aus. Abgewiesene Verbindungen zählt und meldet er wie abgewiesene Sockets (HUM-138).
2. Die Tür des Shims prüft die Gegenseite auf Loopback ebenso über ihre PID und nimmt nur Prozesse derselben Sitzung oder einer weiteren Shell der Sitzung an.
3. Die Escape-Suite unter macOS bekommt Proben: ein mit `setsid` gelöster Prozess erreicht den Proxy nach dem Ende der Sitzung nicht mehr, ein Prozess außerhalb der Sandbox erreicht die Tür nicht.
4. `docs/SECURITY.md` Abschnitt 12, K-19 und K-20 werden entsprechend eingeschränkt.

## HUM-332 · Kleinere Folgen aus den Reviews von M12

1. Die Titel im Register der Codes nennen unter macOS systemd (`DAEMON_008`, `DOCTOR_005`); sie werden plattformneutral oder je Plattform.
2. Kopierbare Befehle nennen unter macOS `humanitl` ohne Pfad, der Befehl liegt aber nur im Bündel; sie nennen den Pfad aus `installServiceCandidate` oder die Anleitung zum Verlinken.
3. Die Ampel des Fensters sitzt etwa 6 px über der Mitte der 40 px hohen Kopfleiste; im Vollbild bleibt die Lücke von 72 px. `MainFlutterWindow` richtet die Knöpfe aus, die Kopfleiste kennt den Vollbildzustand.
4. Hinweise auf Tastenkürzel zeigen unter macOS „Ctrl"; sie zeigen ⌘, die `Ctrl`-Form bleibt wirksam.
5. Eine App aus Downloads startet unter macOS aus einer vorübergehenden Kopie (App Translocation); `daemon install` erkennt das und verweigert mit Hinweis auf `/Applications`.
6. Die Prüfung neuer macOS-Versionen: eine CI-Matrix über `macos-14`, `macos-15` und `macos-latest` für die Escape-Suite.
7. Ein Probe für unprivilegiertes ICMP (`SOCK_DGRAM`, `IPPROTO_ICMP`) im Shim und in der Escape-Suite.

## HUM-333 · Die Website humanitl.de

Wunsch des Inhabers vom 2026-10-30: eine öffentliche Seite unter humanitl.de, gebaut von GitHub Actions und über GitHub Pages ausgeliefert (Domain, Zertifikat und Pages-Modus „GitHub Actions“ sind eingerichtet). Technik und Aufbau wie whowolf.de (Astro, statisch, Deutsch unter `/`, Englisch unter `/en/`, Impressum von nurkert.de), das Aussehen nach den Vorlagen des Inhabers: tiefschwarzer Raum, schwebende 3D-Glasfenster mit weißen Glanzkanten, Chrom-Überschrift, umrandete Pillen, kein Cyan und kein Magenta.

Akzeptanzkriterien:

- `website/` baut mit `npm ci && npm run build` ohne Fehler; `.github/workflows/website.yml` baut bei Änderungen an `website/`, bei jedem veröffentlichten Release und täglich und liefert nach Pages aus, mit festen Commit-Ständen der Actions und nur den Rechten `contents: read`, `pages: write`, `id-token: write`.
- Startseite: Die Fenster der App fliegen beim Laden ein und stehen als Fächer unter der Überschrift. Ein angehefteter Abschnitt morpht sie beim Scrollen auf eine gebogene Bühne mit Metallring; danach rastet jedes Stück Scrollweg ein Modul ein, und erst nach dem letzten scrollt die Seite weiter. Pfeile, Wischen und Pfeiltasten drehen den Ring ohne Ende weiter.
- Die Fenster zeigen echte Bildschirme der App (aufgenommen gegen `humanitld --fake fixtures/sessions/npm-install.jsonl`), keine erfundenen Oberflächen. Jede Aussage der Seite deckt sich mit `README.md` und `docs/SECURITY.md`; es gibt keine erfundenen Zahlen, Nutzer oder Zitate.
- Download-Seite: aktuelle Version, Dateien je System mit Größe, Download-Zahlen aus der GitHub-API (beim Bauen geholt, nie vom Browser des Besuchers), Prüfsummen, erste Schritte mit kopierbarem Befehl. Das System des Besuchers ist vorgewählt; ohne Skript stehen beide Felder offen.
- Impressum (beim Bauen von `IMPRINT_URL` geholt und auf wenige Tags reduziert) und Datenschutzerklärung; keine Cookies, kein Tracking, keine Inhalte Dritter, eine Content-Security-Policy ohne Inline-Skripte.
- Bewegung nur über `transform` und `opacity`, gerechnet nur bei Änderungen; `prefers-reduced-motion` schaltet Einflug und weiche Übergänge ab; ohne Skript zeigt die Startseite den Fächer aus CSS.

Fallstricke: Die Bilder aus `docs/img/` sind Golden-Bilder mit Blöcken statt Text und taugen nicht für die Seite. Das Abzeichen „fake“ in der Statusleiste der App ist abgeschnitten, die Bilder enden über der Statusleiste.

Nicht in diesem Issue: weitere Abschnitte (Bento-Kacheln, Inspector), Doku-Seiten aus `docs/guides/`.

## HUM-334 · Die Bühne von humanitl.de: Glas, Licht, Einflug und Morph

Rückmeldung des Inhabers vom 2026-10-30 zur ersten Fassung (HUM-333): Die Spiegelung liegt so stark auf den Bildschirmen, dass man sie kaum sieht; der Morph vom Fächer zur Bühne ist nicht durchdacht; das Karussell soll nicht am Scrollweg hängen. Gewünscht ist ein elegantes, nicht gezwungenes Motion Design mit einem gut gewählten Bildausschnitt in jedem Seitenverhältnis.

Akzeptanzkriterien:

- Die Bildschirme sind klar lesbar: Glanz und Spiegelung liegen auf dem Rahmen und als feiner Hauch über dem Glas, nie als Schleier, der den Inhalt verdeckt.
- Die Fenster haben eine sichtbare, glänzende Dicke (eine Glas- oder Metallkante, die in der Drehung als Körper erkennbar ist), ohne Bitmap-Texturen.
- Beim Laden fliegen die Fenster nacheinander, gedreht und gestaffelt in ihre Plätze, so dass man den Aufbau sieht; die Bewegung wirkt ruhig und elegant.
- Der Morph vom Fächer auf die Bühne ist choreografiert: jedes Fenster auf einer eigenen, stetigen Bahn ohne Durchdringungen oder Sprünge, mit Licht, das die Bewegung begleitet.
- Nach dem Morph scrollt die Seite normal weiter. Das Karussell dreht nur über Pfeile, Wischen, Ziehen, waagrechtes Scrollen und die Pfeiltasten, ohne Ende in beide Richtungen.
- Jedes Seitenverhältnis bekommt einen eigenen, bewusst gewählten Bildausschnitt: Breitbild, 16:10, 4:3, Hochformat-Tablet und Telefon. Auf dem Telefon darf die Bühne ganz anders aufgebaut sein.
- Bewegung pro Frame nur über `transform` und `opacity`; `prefers-reduced-motion` ohne Einflug und mit sofortigen Übergängen; ohne Skript ein ruhiger Fächer; keine Inline-Skripte (CSP).
- Inhalt und Texte bleiben, wie sie in HUM-333 wahr gemacht wurden.

Nicht in diesem Issue: neue Abschnitte, neue Texte, neue Bildschirmaufnahmen.

## HUM-335 · Linux: das AppImage in jedem Release, APT und AppImage auf der Download-Seite

Wunsch des Inhabers vom 2026-10-30: Das Linux-Release soll ein AppImage mitliefern, und die Download-Seite soll den Weg über das APT-Repository `apt.nurkert.de` zeigen, der automatische Updates bringt. Bisher war das AppImage für 0.1.0 vorgesehen (HUM-060); es kommt jetzt schon in die 0.0.x-Releases. HUM-060 behält VERSION-Datei, Changelog und minisign.

Akzeptanzkriterien:

- `release.yml` baut im Job `build` aus denselben Programmen und demselben Flutter-Bundle wie das `.deb` mit `packaging/appimage/build-appimage.sh` das Bild `Humanitl-<version>-x86_64.AppImage`; `SHA256SUMS` enthält es.
- Ein eigener Job `check-appimage` prüft es mit `packaging/appimage/check-appimage.sh` auf einem frischen Runner, ohne FUSE und ohne Installation; der Job `release` wartet auf ihn und hängt das Bild an das Release.
- Die Release-Notes zeigen den Weg über das AppImage und sagen nicht mehr, dass es fehlt (`release-notes-test.sh` grün).
- Die Download-Seite führt das AppImage als eigene Datei unter Linux und zeigt als empfohlenen Linux-Weg `curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh`, mit einem Hinweis auf das Skript und das `.deb` für alle, die keiner Pipe in eine Root-Shell trauen. Die Aussagen decken sich mit `docs/INSTALL.md`.
- Ein Probelauf von `release.yml` (`workflow_dispatch`) auf dem Issue-Branch ist grün, bevor gemergt wird; der AppImage-Bau läuft sonst in keiner CI.

Fallstricke: `build-appimage.sh` lädt `appimagetool` und die Laufzeit selbst, auf Fassung und Prüfsumme gepinnt; ohne Netz scheitert der Bau. Ein Release, das `release.yml` mit `GITHUB_TOKEN` veröffentlicht, startet keinen anderen Workflow (HUM-333).

## HUM-336 · Die Dokumentation aus docs/ auf humanitl.de

Wunsch des Inhabers vom 2026-10-30: Die Markdown-Dokumentation des Repositorys soll auf der Website durchstöbert werden können und schön gerendert sein.

Akzeptanzkriterien:

- Jede Markdown-Datei unter `docs/` (Anleitungen, Referenz, ADRs, Sicherheit, Architektur und die übrigen) erscheint als eigene Seite unter `/docs/`, aus genau der Datei im Repository gebaut, ohne Abschrift. Eine Startseite `/docs/` ordnet sie in Gruppen.
- Navigation: eine Seitenleiste mit allen Gruppen und Seiten, die aktuelle Seite markiert; je Seite ein Inhaltsverzeichnis der Überschriften; auf dem Telefon eine eigene, bedienbare Form der Navigation. Vor- und Zurück-Verweise zwischen den Seiten einer Gruppe.
- Darstellung im Stil der Website (tiefschwarz, Geist, ruhige Lesespalte): Überschriften mit Ankern, Code mit Hervorhebung und Kopierknopf, Tabellen, die auf schmalen Schirmen waagrecht scrollen, GitHub-Hinweisboxen (`> [!NOTE]` usw.), Bilder aus `docs/img/`, deutschsprachige Seiten mit `lang="de"`.
- Verweise: relative Links zwischen Dokumenten zeigen auf die Seiten der Website (mit Anker), Links auf andere Dateien des Repositorys auf GitHub; kein toter interner Link, kein toter Anker im Bau.
- `website.yml` baut auch bei Änderungen unter `docs/`. Die Navigation der Website führt zu den Docs, die Sitemap enthält sie.
- Keine Inline-Skripte (CSP), kein Laden von Dritten; Bewegung nur über `transform` und `opacity`.

Nicht in diesem Issue: eine Suche über alle Seiten, Übersetzungen der Dokumente.

## HUM-345 · Die Sicherheitsseite von humanitl.de

Ziel: Eine eigene Seite (/sicherheit/, /en/security/) zeigt das Sicherheitsprinzip und
seine Beweisbarkeit: den Weg einer Anfrage als Scroll-Erzählung, die drei Garantien mit
Prüfbefehl, Ausgabe und Ausbruchsproben, die Ausbruchstests, die Audit-Kette mit dem, was
sie beweist und was nicht, die offenen Kanäle und die ehrlichen macOS-Unterschiede.
Bewegung und Anmutung auf dem Niveau der Startseite. Der Nav-Eintrag „Sicherheit“ führt
auf die neue Seite.

Abschnitte: Prinzip, Weg, Beweise, Prüfen, Tests, Aufzeichnung, Grenzen, macOS, Abschluss.

Dateien: website/src/components/pages/Security.astro, website/src/components/security/*,
website/src/scripts/security.ts, website/src/styles/security.css; minimal ui.ts,
[...route].astro, locales (Schlüssel securityPage), Base.astro (ein Nav-Eintrag).

Akzeptanz:
- Jede Aussage ist mit README.md, docs/SECURITY.md, docs/THREAT-MODEL.md,
  docs/ARCHITECTURE.md, ADR-0019 oder tests/escape belegt und verlinkt.
- Linux-Evidenz erscheint nur als gekennzeichnetes Format, nie als echte Ausgabe; nicht
  behauptet wird, Linux-Releases führten die Ausbruchssuite aus; SECURITY.md §5.6 Punkt 8
  wird nicht zitiert; die offenen Kanäle stehen sichtbar, nicht nur in Ausklappern.
- Keine Inline-Skripte, keine Fremdanfragen; Animation pro Bild nur über transform und
  opacity; Animationen laufen nur im Sichtbereich.
- prefers-reduced-motion und ohne JavaScript: vollständige, statische Diagramme, volle
  Lesbarkeit.
- Diagramme mit role="img" und Textäquivalent.
- 360x740 bis 2560x1080 ohne waagrechtes Seitenscrollen; Deutsch und Englisch.

Folge-Kandidaten: Home-Link „Die Sicherheitsargumentation“ auf die neue Seite umbiegen;
SECURITY.md §5.6 Punkt 8 an Seatbelt anpassen; K-22-Satz in THREAT-MODEL.md reparieren.

## HUM-344 · Eine README, die das Projekt zeigt

### Kontext

Rückmeldung des Inhabers vom 2026-10-30: Die README sieht nach Anfängerprojekt aus, obwohl 0.1.0 kurz bevorsteht und schon viel bietet. Gewünscht ist eine saubere, professionelle README mit echten Screenshots. Bisher stand oben ein Golden mit Ahem-Blöcken (`docs/img/intercept.png`), dazu Installationsangaben mit einem AppImage, das in keinem Release liegt.

### Spezifikation

- `README.md` behält die zehn Abschnitte zweiter Ebene aus HUM-059 in derselben Reihenfolge; alles Neue steht darunter als dritte und vierte Ebene. Vor dem ersten Abschnitt: App-Symbol, Einzeiler wortgleich, vier Abzeichen (CI, Release, Lizenz, Plattform), eine Linkzeile (Website, Download, Dokumentation, Sicherheitsmodell, Deutsch), das Intercept-Bild als `<picture>` mit heller und dunkler Fassung, der bisherige Absatz zur Funktionsweise und „How it works in three steps".
- Die Sicherheitsaussage bleibt mindestens so stark: die drei Garantien unter Linux wortgleich mit beiden Prüfbefehlen, eine Tabelle für macOS nach `docs/SECURITY.md` Abschnitt 12, die Escape-Tests ESC-1 bis ESC-5 und die Seatbelt-Suite, die fünf Seitenkanäle als Tabelle, die macOS-Grenzen K-19 bis K-23, „Out of scope" nach `docs/SECURITY.md`. Neu ist der Hinweis auf `docs/SECURITY.md` Abschnitt 5.6 (Antworten und WebSocket-Frames nicht angehalten, kein HTTP/2, Isolationsprüfung erst nach dem Start). `docs/SECURITY.md` und `docs/THREAT-MODEL.md` bleiben unverändert.
- Installation: APT-Repository, `.deb`, Archiv, macOS-Abbild mit „Open Anyway" und einer ausformulierten Warnung zu `xattr`, Prüfsummen mit dem Satz aus `packaging/release/release-notes.sh`. Kein AppImage, solange HUM-335 nicht auf `main` und in einem Release ist.
- Eine Tour durch die App (Intercept, Editor, Regeln, Verlauf und Audit, Sandbox, Tastatur), CLI mit Paritätstabelle, Dokumentationsübersicht, Architekturbild mit Textfassung in `<details>`, Status mit der Zeile M12 und „Project health" als Liste.
- Bilder unter `images/readme/`, nur PNG, zusammen unter 3 MB: echte Aufnahmen der macOS-App gegen `humanitld --fake fixtures/sessions/npm-install.jsonl`, ohne sichtbare Spuren des Demo-Modus, das Architekturbild aus HTML gerendert, das Symbol aus `website/public/favicon.svg`. Editor, Isolationsprüfung, Einrichtung und Audit zeigt der Demo-Daemon nur mit „fake"-Hinweisen; diese Module bleiben ohne Bild.
- `README.de.md` trägt dieselben Fakten zu Garantien, macOS-Grenzen, Seitenkanälen und Installation, kürzer und ohne Tour; für Tour, CLI, Architektur und Stand verweist sie auf die englische README. Das weicht bewusst von HUM-059 („eine Seite") ab: Die deutsche Fassung soll Einsteigern die Sicherheitsaussage und die Installation vollständig geben, die Pflege der langen Teile bleibt an einer Stelle.
- Maschinelle Leser: `scripts/ci/lint-docs.sh` sucht das Bild jetzt als `images/readme/intercept-dark.png` (Markdown-Bild oder `src="…"`), `scripts/ci/check-doc-links.py` prüft auch jedes Ziel in `srcset`. Die Status-Tabelle behält ihr Format für `packaging/release/release-notes.sh`.

### Akzeptanzkriterien

- `scripts/ci/lint-docs.sh`, `python3 -I scripts/ci/check-doc-links.py` (mit `--self-test`) und `packaging/release/release-notes-test.sh` sind grün.
- Die Bildregel in `lint-docs.sh` und `srcset` in `check-doc-links.py` ändern sich im selben Commit; der Selbsttest hat einen Fall für ein fehlendes `srcset`-Ziel.
- Jedes Bild ist eine echte Aufnahme oder aus Fakten des Repositorys gerendert; kein Bild zeigt Text des Demo-Modus.
- `images/readme/` ist zusammen höchstens 3 MB groß, nur PNG oder JPEG.
- Die Sicherheitsaussage ist mindestens so stark wie vorher; K-19 bis K-23 und die fünf Seitenkanäle sind genannt.
- Kein AppImage, kein Link auf `humanitl.de/en/docs/`, kein Abzeichen, das etwas Unprüfbares behauptet.
- Gerendert mit `gh api /markdown` bei 1280 und 400 px, hell und dunkel, ohne waagrechtes Scrollen geprüft.

Offen für den Maintainer: `docs/img/intercept.png` ist danach nirgends mehr verlinkt und kann weg. `images/banner.jpg` bleibt ungenutzt; ob es die Social Preview von GitHub wird, entscheidet der Inhaber.

## HUM-342 · Die Ampel unter macOS sitzt mittig in der Titelleiste der App

Rückmeldung des Inhabers vom 2026-10-30: Die nativen Fensterknöpfe (Ampel) kleben links oben in der 28 pt hohen Titelleiste des Systems und fluchten nicht mit Logo und Titel der 40 pt hohen Kopfleiste der App. Aus HUM-332 („Ampel-Abstand").

Akzeptanzkriterien:

- Die drei Knöpfe stehen senkrecht mittig auf der Kopfleiste der App (`HSize.headerBar`) und so weit vom linken Rand, dass sie mit dem Rest der Leiste fluchten; `trafficLightInset` reicht weiter für sie.
- Sie bleiben dort nach Größenänderung, Fokuswechsel, Bildschirmwechsel und nach dem Verlassen des Vollbilds; im Vollbild bleibt die Leiste des Systems unberührt.
- Die Titelleiste des Systems wird nicht vergrößert, damit sie keine Klicks auf Elemente der App abfängt.
