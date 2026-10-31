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
| HUM-343 | Der Ordnerdialog öffnet sich unter macOS | S | HUM-325, HUM-326 |
| HUM-346 | Die Docs zweisprachig auf humanitl.de | M | HUM-336 |
| HUM-339 | Linux: ein RPM für Fedora und openSUSE und das AUR-Paket humanitl-bin | M | HUM-335 |
| HUM-337 | Download: der Knopf nennt das System, keine Empfehlung, neutrale Symbole, eine Tafel für die Zahlen | S | HUM-333, HUM-335 |
| HUM-347 | Bühne: größeres Fokusfenster, Ring danach ausgerichtet, Tiefenschärfe nach hinten, Pfeile immer erreichbar | M | HUM-334 |
| HUM-349 | Der Zugangsschlüssel unter macOS im Keychain | S | HUM-140, HUM-323 |
| HUM-357 | macOS: pi liest den Modellschlüssel auch aus einem Pfad mit Leerzeichen | S | HUM-328 |
| HUM-348 | Landing: wenige Scheiben seitlich geneigt mit sichtbarer Tiefe, Licht wie geschliffenes Glas | M | HUM-347 |
| HUM-359 | Website: Aussagen, die Code und Doku nicht decken, und falsche Sprachlinks | S | HUM-333, HUM-345, HUM-346 |
| HUM-358 | Startseite: sechs Sektionen unter dem Karussell, die erklären, überzeugen und die Unterseiten anteasern | M | HUM-333, HUM-345, HUM-359 |
| HUM-341 | Landing: dezenter Nebel mit Lichtstrahlen hinter den Fenstern | S | HUM-334, HUM-348 |
| HUM-338 | Website: heller Modus, folgt dem System, Umschalter in der Navigation | M | HUM-341, HUM-358 |
| HUM-361 | Flaky-Test: `a_stalled_streamed_upload_ends_with_limit_004` nimmt den Abbruch nach der Antwort hin | S | HUM-057 |
| HUM-360 | Website auf Telefonen: Download-Felder passen in die Breite, Pfeile und Zählwerk der Bühne stehen frei | S | HUM-337, HUM-347, HUM-338 |
| HUM-364 | Proxy: Klartext-HTTP im `CONNECT`-Tunnel, damit `pi` 1.x sein lokales Modell erreicht | S | HUM-024 |
| HUM-367 | Statusleiste: Daemon-Neustart auch mit laufender Sitzung, ohne `systemctl`-Hinweis | S | HUM-284 |
| HUM-365 | macOS: OpenCode 2 startet in der Sandbox, mit einem eigenen Port je Sitzung | M | HUM-323 |
| HUM-368 | macOS: Das Sitzungsverzeichnis samt Zugangsschlüssel verschwindet mit der Sandbox | S | HUM-323 |

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

## HUM-343 · Der Ordnerdialog öffnet sich unter macOS

Rückmeldung des Inhabers vom 2026-10-30: Unter macOS öffnet „Ordner wählen" (Projektordner der Sandbox) keinen Dialog. Ursache: `file_picker_darwin` öffnet `NSOpenPanel` nur, wenn die App das Entitlement `com.apple.security.files.user-selected.read-only` (oder read-write) trägt, auch ohne App-Sandbox; sonst gibt es einen Fehler zurück. Dazu signierte `packaging/macos/build-app.sh` das Bündel mit `--deep` neu und hätte jedes Entitlement entfernt.

Akzeptanzkriterien:

- Beide Entitlement-Dateien tragen `com.apple.security.files.user-selected.read-only`; die App bleibt ohne App-Sandbox.
- `build-app.sh` signiert die App zuletzt mit `Release.entitlements` und bricht ab, wenn das Entitlement im fertigen Bündel fehlt; das gilt auch für das `.zip` nach einem einfachen `unzip`.
- Der Ordnerdialog öffnet sich in der gebauten App.

Nicht in diesem Issue: den Projektordner beim Start der Sandbox optional zu machen. Der Inhaber hat am 2026-10-30 entschieden, dass der Ordner vorerst Pflicht bleibt; eine Sitzung ganz ohne Ordner (leeres `/work`) kann später sinnvoll sein und wird dann ein eigenes Issue.

## HUM-346 · Die Docs zweisprachig auf humanitl.de

Ziel: Die Docs auf humanitl.de sind sauber auf Deutsch und auf Englisch zu lesen, wie der
Rest der Seite: Deutsch unter /docs/, Englisch unter /en/docs/. Die Originale in docs/
bleiben unverändert; die Übersetzungen liegen daneben und tragen den Stand des Originals,
auf dem sie beruhen, damit eine veraltete Übersetzung erkennbar bleibt.

Festgelegt:
- Übersetzungen liegen unter `docs/translations/de/<Pfad wie im Original>` (deutsche
  Fassungen englischer Originale) und `docs/translations/en/<Pfad>` (englische Fassungen
  von `CONFIG.md`, `DIAGNOSTICS.md`, `reference/parity.md`). Front Matter mit genau
  `source`, `source-sha256` (SHA-256 der Bytes des Originals beim Übersetzen) und `lang`.
- Eine Übersetzung behält die Gliederung ihres Originals (Zahl, Ebene und Reihenfolge der
  Überschriften), jedes Linkziel, jeden Bildpfad, jeden Anker, jeden Codeblock Byte für
  Byte, Bezeichner, Tabellen- und HTML-Struktur. Begriffe nach `docs/GLOSSARY.md`, zitierte
  App-Texte nach `app/l10n/app_en.arb` und `app_de.arb`.
- Fehlt eine Übersetzung oder ist sie veraltet, bricht der Bau nicht ab; die Seite zeigt
  einen Hinweis, und `website/scripts/check-translations.mjs` listet beides.

Dateien: docs/translations/**, website/src/content.config.ts, website/src/lib/docs.ts,
docs-catalog.mjs, remark-docs.mjs, rehype-docs.mjs, website/src/layouts/Docs.astro,
website/src/components/docs/{DocPage,DocsHome,LocalizedLinks,DocsNav}.astro,
website/src/pages/docs/*, website/src/pages/en/docs/*, website/src/pages/sitemap.xml.ts,
website/src/scripts/docs.ts, website/src/styles/docs.css, Block `docs` in den Locales,
Base.astro (Sprachwahl und Docs-Link je Sprache), website/scripts/check-links.mjs,
website/scripts/check-translations.mjs, website/scripts/docs-markdown.test.mjs,
.github/workflows/website.yml.

Akzeptanz:
- Jede Datei unter docs/ hat eine Seite unter /docs/<slug>/ und unter /en/docs/<slug>/; die
  Seite zeigt in der Sprache des Originals das Original, in der anderen Sprache die
  Übersetzung. `<html lang>` und `lang` des Artikels nennen die Sprache des gezeigten Textes.
- Jede Überschrift einer Übersetzung trägt die ID der Überschrift an derselben Stelle im
  Original. Der Bau prüft Zahl und Ebenen; weichen sie ab, behält die Übersetzung ihre
  eigenen IDs, der Bau meldet es mit einer Warnung, und `check-translations.mjs` nennt die
  Datei.
- Relative Verweise einer Übersetzung werden relativ zum Original aufgelöst und zeigen auf
  die Seite derselben Sprache, mit Anker; Bilder laden dieselbe Datei wie das Original.
- Jede Docs-Seite hat die Sprachwahl DE/EN der Website; sie führt auf dieselbe Seite und,
  mit JavaScript, an dieselbe Stelle der anderen Sprache. hreflang-Paare im Kopf und in der
  Sitemap, x-default ist die deutsche Seite.
- Fehlt eine Übersetzung, zeigt die Seite das Original mit dem sichtbaren Hinweis „noch
  nicht übersetzt" / „not translated yet"; Seitenleiste und Startseite kennzeichnen sie mit
  dem Kürzel der Originalsprache. Passt `source-sha256` nicht zum Original, zeigt die Seite
  die Übersetzung mit dem Hinweis, dass sich das Original geändert hat, und verweist auf
  das Original.
- Startseite der Docs in beiden Sprachen, mit übersetzten Gruppen, Beschriftungen und der
  Einleitung der README in der Sprache der Seite.
- `check-links.mjs` prüft zusätzlich: Gegenstück in der anderen Sprache, `<html lang>`,
  hreflang, Sprachwahl, und dass kein Artikel in die Docs der anderen Sprache verweist.
- `check-translations.mjs` listet fehlende, veraltete, abweichend gegliederte, verwaiste und
  ungültige Übersetzungen und endet immer mit Status 0; website.yml ruft es nach dem Bau auf.
- Tests in `docs-markdown.test.mjs` für die Zuordnung der IDs (auch den Rückfall) und für
  die Verweise einer Übersetzung; 1440x900 und 390x844 ohne waagrechtes Scrollen.

Offen außerhalb des Umfangs: `scripts/ci/lint-docs.sh` (H1 auf Zeile 1, `Applies to` auf
Zeile 3) und `scripts/ci/check-doc-links.py` (Links relativ zur eigenen Datei) müssen
`docs/translations/` ausnehmen oder relativ zum Original prüfen, sonst schlägt die CI fehl.
Die Sicherheitsseite (HUM-345) verlinkt auf der englischen Fassung noch /docs/security/
statt /en/docs/security/.

## HUM-339 · Linux: ein RPM für Fedora und openSUSE und das AUR-Paket humanitl-bin

Wunsch des Inhabers vom 2026-10-30: Linux-Pakete über das `.deb` hinaus, zuerst ein RPM (Fedora, openSUSE) und ein AUR-Paket für Arch. Den Upload ins AUR und ein dnf-Repository neben `apt.nurkert.de` übernimmt der Inhaber selbst.

Akzeptanzkriterien:

- `release.yml` baut `humanitl-<version>-1.x86_64.rpm` aus denselben Programmen, demselben Flutter-Bundle und demselben Dateibaum wie das `.deb` (`packaging/rpm/build-rpm.sh`), reproduzierbar mit `SOURCE_DATE_EPOCH`; `SHA256SUMS` enthält es, das Release hängt es an.
- Ein Job `check-rpm` installiert es in einem auf Digest gepinnten Fedora-Container, prüft Dateien, Versionen, Units, Desktop-Datei und dass `humanitl daemon install --print` das Paket erkennt, und entfernt es ohne Reste; `rpmlint` ohne Fehler und Warnungen, jede gefilterte Meldung mit Begründung.
- Die Scriptlets entsprechen den Maintainer-Skripten des `.deb` (Neuladen und Neustart der Nutzerdienste, nie ein Fehlschlag der Transaktion); `packaging/rpm/scriptlets-test.sh` läuft in `make deps-lint`.
- `packaging/aur/render.sh` erzeugt PKGBUILD und `.SRCINFO` für ein Release aus dem Tarball oder seiner Prüfsumme; ein Job `check-aur` baut das Paket in einem gepinnten Arch-Container mit `makepkg`, installiert und entfernt es. `packaging/aur/README.md` beschreibt den Upload durch den Inhaber.
- Release-Notes, `docs/guides/install.md`, `docs/INSTALL.md` und `CONTRIBUTING.md` beschreiben RPM und AUR wahrheitsgemäß, mit der glibc-Grenze 2.39 (Fedora 40 und neuer, Tumbleweed; RHEL 9 und Leap 15 nicht). Die deutschen Übersetzungen der beiden Docs ziehen mit.
- Die Download-Seite führt das RPM mit `dnf install` und nennt das AUR-Paket nur, wenn das AUR es beim Bauen kennt.

## HUM-337 · Download: der Knopf nennt das System, keine Empfehlung, neutrale Symbole, eine Tafel für die Zahlen

Wünsche des Inhabers vom 2026-10-30.

Akzeptanzkriterien:

- Die Download-Knöpfe der Startseite nennen das System des Besuchers. Auf dem Mac führen sie direkt zum Disk-Image der aktuellen Version, unter Linux zur Download-Seite, weil es dort mehrere Wege gibt. Auf anderen Geräten, ohne Skript und wenn beim Bauen kein Disk-Image bekannt ist, bleiben sie neutral.
- Die Download-Seite spricht keine Empfehlung aus; sie öffnet still das Feld des Systems.
- Apple-Logo und Tux sind entfernt (das Apple-Logo darf ohne Lizenz von Apple nicht stehen). macOS und Linux tragen selbst gezeichnete Symbole in der Strichstärke der Website; `platform-icons.ts` und sein Eintrag in `THIRD_PARTY.md` entfallen.
- Version und Download-Zahlen stehen auf einer Tafel. Gezählt werden Installationsdateien aller Versionen; ein Balken teilt sie nach System auf, die Teile ergeben die Summe. Er wächst nur über `transform`.

## HUM-347 · Bühne: größeres Fokusfenster, Ring danach ausgerichtet, Tiefenschärfe nach hinten, Pfeile immer erreichbar

Wünsche des Inhabers vom 2026-10-30.

Akzeptanzkriterien:

- Im Karussell nimmt das Fokusfenster deutlich mehr Platz ein, auf Desktop-Formaten rund 25 bis 35 Prozent mehr Breite als vorher, soweit Rand und Höhe es zulassen. Der Ring ist dafür neu ausgerichtet; das Fenster schneidet die Beschriftung im Ruhezustand nicht.
- Die hinteren Fenster werden nach hinten zunehmend unscharf, mit Bokeh-Scheiben. Das Fokusfenster bleibt scharf, ebenso alle Fenster in der Landing-Section.
- Kein Filter wird pro Frame neu berechnet: die Unschärfe liegt fest in einer eigenen Ebene, das Skript setzt nur `opacity` und `transform`. Mit `data-lite` entfällt das Bokeh; ohne Skript und mit reduzierter Bewegung bleibt die Seite vollständig.
- Die Pfeile erscheinen, sobald der Ring steht, sind ohne Suchen sichtbar und liegen in jeder Layout-Klasse im Bild und klickbar, über alle Module. Drag und Tastatur bleiben daneben bestehen.

## HUM-349 · Der Zugangsschlüssel unter macOS im Keychain

### Kontext

Der Inhaber sah in der macOS-App unter Einstellungen „Set, in a file only you can read: the system keyring did not answer" und dazu `secret-tool is not installed`: `daemon/crates/config/src/secret.rs` kannte nur das Linux-Werkzeug, und der Text riet zu einem Paket von Debian. Unter macOS blieb der Schlüssel deshalb immer in der Datei.

### Spezifikation

- Unter macOS liegt der Schlüssel im Anmelde-Schlüsselbund, gesprochen wird mit `/usr/bin/security`. Kein Crate: Die App ist nur ad-hoc signiert, ein Eintrag, dessen Zugriffsliste unserem Programm vertraut, fragte nach jedem Update. Einträge von `security` vertrauen dem von Apple signierten Werkzeug.
- Neues Schema `keychain:humanitl/<name>` neben `secret-service:` und `file:`, weil ein Verweis den Ort nennt. Ein Verweis des anderen Systems meldet `LLM_009` mit `humanitl llm key set`, ohne dort den falschen Speicher zu fragen, und `KeyStore::clear` nimmt ihn als schon weg hin, damit `humanitl llm key remove` ihn aus der Konfiguration nimmt. Dienst `humanitl`, Konto der Schlüsselname, `TOOL_TIMEOUT` und `run_tool` wie bisher.
- Hinein über `security -i` auf `stdin`, nie über `argv`. Gemessen unter macOS 27 an einem Wegwerf-Schlüsselbund: Eine Zeile über gut 4096 Bytes wird zerschnitten und der Rest als Befehl gelesen, samt Ende des Schlüssels auf `stderr`; eine fehlerhafte Zeile kann mit 0 und ohne Ausgabe enden; `find-generic-password -w` gibt Nicht-ASCII als Hexadezimaltext aus und hängt `\n` an; ein fehlender Eintrag endet mit 44. Darum: Zeilengrenze 4000 Bytes, nur druckbares ASCII, Zurücklesen und Vergleich nach dem Ablegen, und aus `stderr` von `-i` nur Zeilen `security: SecKeychain…` in den Befund.
- Rückfall in die `0600`-Datei mit `CONFIG_019`, wenn `security` nicht nimmt; der Text nennt „the macOS Keychain", der Vorschlag ist `security unlock-keychain`, kein `secret-tool`, kein `libsecret-tools`. Bei einem Schlüssel, den `security` nie trüge, steht kein Vorschlag da. Texte unter Linux bleiben Byte für Byte.
- Der Speicher wählt zur Laufzeit, damit die Tests beider Systeme mit Attrappen auf beiden laufen. `KEYCHAIN_TOOL` gilt nur, wenn der Suchpfad `/usr/bin` nennt, sodass Tests mit Wegwerf-Pfaden den Schlüsselbund des Menschen nicht erreichen. Keine Vertragsänderung: `LLM_KEY_STORE_SECRET_SERVICE` heißt auf dem Draht „Schlüsselbund des Systems", den Unterschied trägt `key_ref`, und die CLI zeigt daraus `keychain`.
- `docs/SECURITY.md`, `docs/THREAT-MODEL.md`, Anleitung, README und Website sagen es ehrlich: Ein Eintrag ist für jeden Prozess desselben Kontos außerhalb der Sandbox mit `security` lesbar, dieselbe Schutzklasse wie der Secret Service. Der Agent erreicht ihn nicht: `MACH_SERVICES` nennt kein `securityd`, `~/Library/Keychains` liegt in `/Users`.

### Akzeptanzkriterien

- Der Schlüssel steht nie in `argv` (`the_keychain_takes_the_key_over_stdin_and_never_over_argv`, Mutationsbeweis: mit `-w` in `argv` rot).
- Anführungszeichen und Backslashes kommen unverändert zurück, gegen die Attrappe und einmalig gegen `/usr/bin/security` mit einem Wegwerf-Schlüsselbund, ebenso Ersetzen mit `-U`, Lesen und Löschen.
- Ein scheiterndes `security -i` führt in die Datei mit `CONFIG_019` (`a_failed_security_i_falls_back_to_the_file_and_says_so`, Mutationsbeweis: Ausgang ignoriert, rot); ein stilles Scheitern fängt das Zurücklesen ab.
- Kein Text unter macOS nennt `secret-tool` oder `libsecret-tools`; kein Befund trägt den Schlüssel, auch nicht aus `stderr`.
- `file:`-Verweise und die Datei wirken weiter; das nächste Setzen zieht den Schlüssel ins Keychain und löscht die Datei.
- Der Test `seatbelt` hält fest, dass `MACH_SERVICES` keinen Dienst des Schlüsselbunds nennt; in einem Profil mit derselben Mach-Liste findet `security` selbst bei lesbarer Keychain-Datei nichts.
- Kein Test schreibt in das Anmelde-Keychain.

Folge-Kandidaten:

- `tests/escape/macos/run.sh` um eine Zeile erweitern, die `security find-generic-password` aus der laufenden Sandbox ruft.
- Nach einem Rückfall in die Datei bleibt ein älterer Eintrag im Schlüsselbund liegen, auf den nichts mehr zeigt. Das gilt seit HUM-140 auch unter Linux und ist hier nicht behoben.

## HUM-357 · macOS: pi liest den Modellschlüssel auch aus einem Pfad mit Leerzeichen

Rückmeldung des Inhabers vom 2026-10-30: Unter macOS scheitert jede Modellanfrage von pi mit „Failed to resolve API key … from shell command: cat /Users/…/Application Support/…". Der Pfad des Schlüssels liegt dort unter `Application Support` und stand ungequotet in der Zeile, die pi durch `/bin/sh -c` schickt.

Akzeptanzkriterien:

- Die Zeile in `models.json` nennt den Pfad nur über die Variable `HUMANITL_PI_KEY_FILE`; Leerzeichen und Apostrophe des Pfads erreichen die Shell-Zeile nicht. Die Variable trägt den Pfad, nie den Schlüssel.
- Ein Test belegt, dass die Variable gesetzt ist und die Abbildung unter macOS sie mit umschreibt.

## HUM-348 · Landing: wenige Scheiben seitlich geneigt mit sichtbarer Tiefe, Licht wie geschliffenes Glas

Wünsche des Inhabers vom 2026-10-30.

Akzeptanzkriterien:

- In der Landing-Section stehen nur drei, höchstens vier Fenster, sauber gestaffelt und seitlich geneigt, sodass Dicke und Z-Tiefe jeder Scheibe sichtbar sind. Die übrigen blenden im Morph auf; alle sieben erreichen den Ring.
- Jedes Fenster liegt vom Licht her unter einer abgerundeten Glasscheibe ohne harte Kante: ein weiches Band folgt dem Rand, läuft ohne Knick um die Ecken und fällt nach innen sanft ab.
- Das ist nur Reflexion über dem Bild. Es gibt keine Brechung, der Screenshot wird nicht verzerrt, und kein Filter läuft pro Frame.
- Der Fächer schneidet weder Text noch Knöpfe, erzeugt keinen waagrechten Überlauf, und die Fassung ohne Skript zeigt denselben Aufbau.

## HUM-359 · Website: Aussagen, die Code und Doku nicht decken, und falsche Sprachlinks

### Kontext

Ein Abgleich von humanitl.de mit `README.md`, `docs/SECURITY.md` und `docs/THREAT-MODEL.md` fand Sätze, die mehr versprechen, als die Software hält. Der Rechtstext sagte, du entscheidest Anfrage für Anfrage; `allow`-Regeln und der Passthrough zum Modell laufen aber ohne Halt. Der Passthrough läuft vor allen Regeln, mitgelieferte Regeln sind nicht „deine". Der Agent sieht unter Linux auch `/usr` und Teile von `/etc`, unter macOS fast alles außerhalb von `/Users` (K-21). Der Modellverkehr ist im Verlauf standardmäßig ausgeblendet, ein Ausfall der Aufzeichnung hinterlässt eine Lücke (K-16). Unter macOS besteht die Tür aus Socket und Loopback-Port (K-19), die Prüfung läuft nach dem Start (K-15), und der Agent kann den Modellschlüssel lesen. Dazu führten die englischen Seiten auf die deutschen Docs.

### Spezifikation

- `website/src/i18n/locales/de.ts` und `en.ts` sagen dasselbe; jeder geänderte Satz ist durch `README.md`, `docs/SECURITY.md` oder `docs/THREAT-MODEL.md` gedeckt.
- `privacy.sections` App nennt freigegebene Anfragen, eingeschaltete Regeln und den Modellaufruf ohne Rückfrage; `privacy.updated` trägt das neue Datum.
- `hero.lede`, `meta.title` und `meta.description` beschreiben Anfragen richtig: Anfragen ans Sprachmodell gehen durch, über jede andere entscheidet eine Regel, oder sie wartet. `hero.title` bleibt, seine Entscheidung liegt beim Inhaber.
- `stage.modules` Sandbox, Verlauf, Modell und Intercept, `guarantees.lede`, `guarantees.items[1]`, `securityPage.proofs.lede`, `securityPage.closing.text` und `download.platforms.macos.req` werden nach den Belegen oben gefasst.
- „Nothing in this document has to be believed." in `docs/SECURITY.md` §9 entfällt, die Übersetzung zieht mit.
- Alle `/docs/…`-Verweise in `en.ts` zeigen auf `/en/docs/…`; `website/src/components/security/Closing.astro` nimmt `docsHome(lang)`.
- `docs/guides/agent-profiles.md`, Abschnitt „Using pi", nennt den Weg unter macOS über das npm aus Homebrew, die Übersetzung zieht mit.

### Akzeptanzkriterien

- `de.ts` und `en.ts` enthalten keinen der beanstandeten Sätze mehr: „Anfrage für Anfrage", „Was keine deiner Regeln erlaubt", „sonst nichts von deinem Rechner", „Nachlesen, was rausging", „Ein einziger Socket führt", „jede mit einem Befehl prüfbar", „Nichts davon muss man glauben", „Getestet mit macOS 15 und 27", jeweils auch nicht in der englischen Entsprechung.
- `hero.title`, `website/src/scripts/stage.ts` und `website/src/components/pages/Home.astro` bleiben unverändert.
- In der gebauten Seite trägt keine Seite unter `/en/security/` einen Verweis auf `/docs/…`; alle Anker lösen auf, `npm run check-links` ist grün.
- Die englische Sicherheitsseite führt in `Closing.astro` auf `/en/docs/security/` und `/en/docs/threat-model/`, die deutsche auf `/docs/security/` und `/docs/threat-model/`.
- `npm run check-translations` führt `docs/translations/de/SECURITY.md` und `docs/translations/de/guides/agent-profiles.md` als aktuell, `scripts/ci/lint-docs.sh` und `scripts/ci/check-doc-links.py` sind grün.

Folge-Kandidaten:

- `scripts/check-links.mjs` könnte auch außerhalb der Doc-Artikel prüfen, dass englische Seiten nur auf englische Docs verweisen.
- `docs/translations/de/DESIGN.md` und `docs/translations/de/UX.md` sind seit HUM-356 veraltet und brauchen dessen Abschnitte zur Skalierung.
- Der Installationshinweis in `daemon/crates/sandbox/src/agent/pi.rs` und `docs/guides/install.md` nennen unter macOS noch `sudo npm`.

## HUM-358 · Startseite: sechs Sektionen unter dem Karussell, die erklären, überzeugen und die Unterseiten anteasern

### Kontext

Die Startseite endete nach dem Karussell mit drei Garantien und einem Abschluss. Wer noch nicht weiß, dass er Humanitl braucht, erfuhr dort weder, was passiert, noch was es kostet, und die Unterseiten blieben ungenannt. Eine simulierte Persona-Befragung (keine Marktdaten) ergab die Reihenfolge: erst ein Angriff, dann die Mechanik, dann was offen bleibt, dann der Preis im Alltag, Unterstützung und Reifegrad. Hero und Karussell bleiben, entgegen dem Rat der Befragung. Stoppzahlen sind nicht gemessen und erscheinen deshalb nicht.

### Spezifikation

- Unter dem Karussell stehen in dieser Reihenfolge sechs Sektionen aus `website/src/components/home/`: ein Angriff, durchgespielt; warum es hält; was offen bleibt; was es im Alltag kostet; Agenten und Systeme; Reifegrad und Download. Sie ersetzen den Garantien-Block und den alten Abschluss; die Ausgabe von `humanitl sandbox check` und der Download-Knopf (`smart-download.ts`) bleiben.
- Texte stehen unter `home` in `website/src/i18n/locales/de.ts` und `en.ts` und sagen dasselbe. Jeder Satz ist durch `README.md`, `docs/SECURITY.md`, `docs/THREAT-MODEL.md` oder den benannten Leitfaden gedeckt.
- Jede Sektion führt mit einem sprachrichtigen Verweis auf ihre Unterseite (Sicherheit `#weg`, `#pruefen`, `#grenzen`, `#macos`, Docs `reference/rules/`, `guides/agent-profiles/`, Download).
- Das Schema einer Anfrage läuft einmal ab und lässt sich wiederholen. Reine Abfolge, keine Dauerschleife.
- Bewegung nur über `transform` und `opacity`, ausgelöst von `website/src/scripts/home.ts` ohne scroll-Listener. Ohne Skript und bei reduzierter Bewegung steht der Endzustand. Farben kommen aus Tokens (`website/src/styles/home.css`).

### Akzeptanzkriterien

- `hero.title`, `website/src/scripts/stage.ts`, Hero und Karussell bleiben unverändert.
- `npx astro build`, `npm test` und `npm run check-links` sind grün.
- Von 320 px bis 3440 px gibt es keinen waagrechten Scroll der Seite.
- Ohne Skript und mit `prefers-reduced-motion: reduce` sind alle Sektionen vollständig sichtbar.
- Die Startseite enthält weder „Physik statt Vertrauen" noch „ehrlich" als Selbstlob, „Garantie" ohne Geltungsbereich oder „jede Anfrage", keine Stoppzahlen, keine Testimonials und keine Fremdlogos.
- „Was offen bleibt" nennt alle fünf erklärten Kanäle aus `README.md` samt ihren Grenzen, und ohne Halt geht nur, was zum erklärten Passthrough passt.
- Das Schema startet auch auf niedrigen Fenstern, Fokus zeigt ein noch nicht aufgetretenes Element sofort, und gedruckt steht der Endzustand.
- Aussagen zu einem externen Audit, zu Cloud-Modellen und zur APT-Signatur fehlen, bis der Inhaber sie entschieden hat.

Folge-Kandidaten:

- Echte Messung der Halte pro Lauf, um die Sektion zum Alltag zu belegen.
- Ein Bildschirm der Regeln mit gespeicherten eigenen Regeln; die vorhandene Aufnahme zeigt nur die mitgelieferten.
- Satz zum externen Audit, zu Cloud-Modellen und zur APT-Signatur, sobald der Inhaber entschieden hat.

## HUM-341 · Landing: dezenter Nebel mit Lichtstrahlen hinter den Fenstern

### Kontext

Wunsch des Inhabers: ganz am Anfang der Startseite etwas leicht beleuchteter Nebel mit Lichtstrahlen. Fenster und Überschrift bleiben der Blickfang, der Raum bleibt dunkel. Die Bühne bewegt pro Bild nur `transform` und `opacity`; dabei bleibt es.

### Spezifikation

- Drei vorgerechnete Ebenen in `website/src/assets/fog/` (`far`, `rays`, `near`), erzeugt von `website/scripts/generate-fog.mjs` aus festem Rauschen mit `sharp`, von Hand und nicht im Build. Die Strahlen folgen `LIGHT` aus `website/src/scripts/stage.ts`. Ausgeliefert über `Picture` als AVIF mit WebP als Rückfall.
- Die Ebenen liegen in `.sky`, also hinter den Fenstern und vor dem Hintergrund. Die Stärke hängt an `--fog-far`, `--fog-near` und `--fog-rays`.
- Eine einmalige CSS-Animation auf `transform` und `opacity` lässt die Ebenen 24 Sekunden im Compositor driften und weich auslaufen; danach hält die letzte Pose ohne Animation und ohne `will-change`, bei verborgenem Dokument pausiert sie. Der Zeiger verschiebt sie über `translate` mit Transition, der Morph über `transform`. Kein Filter, kein Canvas, kein Blend-Modus.
- Die Bilder laden sofort mit `fetchpriority="low"`. Jede Ebene blendet auf, sobald ihr Bild dekodiert ist; der Einflug wartet nicht darauf.
- Der Nebel kommt mit dem Einflug, ist bei `m = 0,3` ganz ausgeblendet und liegt danach nicht mehr im Layout, auch nach einem Sprung am Morph vorbei.
- Bei reduzierter Bewegung steht er still. Mit `data-lite` entfallen Drift, Zeiger und die Anzeige der vorderen Ebene; ihr Bild (rund 4 KB) lädt trotzdem, weil es sonst ein Inline-Skript bräuchte, das die CSP verbietet. Ohne Skript steht er still über dem ersten Bild.

### Akzeptanzkriterien

- `npx astro build`, `npm test` und `npm run check-links` sind grün.
- Jede ausgelieferte AVIF-Datei ist kleiner als 60 KB.
- Während der Drift ohne Eingabe gibt es weder Paint noch Layout noch rAF.
- Nach dem Auslaufen läuft keine Animation mehr, und die Leerlauflast aller Browserprozesse liegt nahe am Stand ohne Nebel.
- Von 320 px bis 3440 px gibt es keinen waagrechten Scroll der Seite.
- Der Kontrast der Lede bleibt über 4,5:1 gegen den hellsten Hintergrund unter ihr.

Folge-Kandidaten:

- Ein helles Thema braucht dunkle Fassungen der Bilder; die Tokens regeln nur die Stärke, die Farbe steckt im Bild.

## HUM-338 · Website: heller Modus, folgt dem System, Umschalter in der Navigation

### Kontext

Wunsch des Inhabers: Die Seite bekommt einen hellen Modus. Ohne Wahl folgt sie dem System, ein Umschalter lässt wählen. Maßstab ist die dunkle Seite; die bleibt, wie sie ist, bis auf `--fg-2` (#737A8C wird #808799), weil der alte Wert die 4,5:1 auf Panels verfehlt. Die App hat ein helles Thema (`HThemeMode`, `app/lib/features/shell/providers/theme.dart`), die Aufnahmen der Bühne bleiben trotzdem dunkel; helle Aufnahmen wären ein eigenes Issue.

### Spezifikation

- Farben hängen an Tokens in `website/src/styles/global.css`. `--dark` und `--light` sind ein Schalterpaar: Das aktive steht auf `initial`, das andere ist leer, darum liefert `var(--dark, a) var(--light, b)` je Thema genau einen Wert, auch für Verläufe, Schatten und Zahlen. Nur der Schalterblock kennt `prefers-color-scheme` und `[data-theme]`. Papier nimmt immer die helle Fassung.
- `website/public/theme.js` lädt blockierend und extern im Kopf, weil die CSP Inline-Skripte verbietet. Es setzt `data-theme` aus `localStorage` (`humanitl-theme`) vor dem ersten Bild, gleicht `theme-color` und `color-scheme` an und setzt `data-theme-js`, das den Umschalter sichtbar macht. Ohne Skript gilt das System, der Umschalter fehlt.
- Der Umschalter steht in der Navigation (`src/scripts/nav.ts`), wechselt zwischen Hell und Dunkel und nennt das Ziel im `aria-label` aus den Locales. Wer beim Stand des Systems ankommt, löst die Wahl; die Seite folgt dem System dann wieder live.
- Bühne im Hellen: Die Fenster bleiben dunkles Glas, die Überschrift wird dunkler Stahl, der Ring dunkler Stahl auf hellem Boden. Hintere Fenster sinken in dunklen Schiefer statt in Schwarz. Bokeh entfällt, ohne Lichtpunkte im Dunkeln ergibt es keinen Sinn. Hinten einkommende Fenster steigen über den CSS-Faktor `--st-ramp` steiler auf, sonst steht eine blasse Geisterscheibe im Raum. Farben kommen nur über CSS, pro Bild bleiben es `transform` und `opacity`.
- Nebel: `far` und `near` gibt es als `far-light.webp` und `near-light.webp` in Schiefer aus `generate-fog.mjs`, die dunklen Fassungen bleiben Byte für Byte gleich. Die Strahlen bleiben weiß. Beide Fassungen laden `lazy`, so dass nur die im Layout liegende geholt wird; jedes Bild blendet einzeln auf, sobald es dekodiert ist, auch nach einem Themenwechsel; das löst die Vorgabe „lädt sofort" aus HUM-341 für diese beiden Ebenen ab.
- Terminals (`.term`, `.sec-term`, `.sec-oneliner`, `.sec-cmd`, `.cmd`) bleiben in beiden Themen dunkle Gegenstände. Codeblöcke der Docs werden hell, dort zählt das Lesen.
- `--accent` bleibt `#E8B84B` für Flächen. Gelbe Schrift nimmt `--accent-ink`, im Hellen `#7A5600`.
- Bis 380 px Breite entfällt in der Navigation die Wortmarke, damit Sprachwahl, Umschalter und Download-Knopf von 320 px an ungeschnitten stehen.

### Akzeptanzkriterien

- `npx astro build`, `npm test` und `npm run check-links` sind grün.
- Text erreicht in beiden Themen mindestens 4,5:1, große Schrift 3:1, gemessen an gerenderten Pixeln.
- Mit gespeicherter Wahl gegen das System zeigt schon das erste Bild die gewählte Farbe.
- Ohne Wahl folgt die Seite einem Systemwechsel ohne Neuladen.
- Ohne Skript gilt das System und der Umschalter fehlt; gesperrter Speicher wirft keinen Fehler.
- Von 320 px bis 3440 px gibt es keinen waagrechten Scroll, der Download-Knopf ist bei 320 px ganz sichtbar.

Folge-Kandidaten:

- Helle Aufnahmen der App für die Bühne, die Karussellbilder und die Download-Seite.
- `og.jpg` zeigt weiter die dunkle Bühne.

## HUM-361 · Flaky-Test: `a_stalled_streamed_upload_ends_with_limit_004` nimmt den Abbruch nach der Antwort hin

### Kontext

CI-Lauf 41155320737 fiel in `daemon/crates/proxy/tests/limits.rs:1111` mit `readable: Os { code: 104, kind: ConnectionReset }`, der Lauf danach bestand. Zu klären war, ob der Test wettläuft oder der Proxy die Antwort mit `LIMIT_004` verliert.

### Befund

Der Proxy verliert nichts, die Ursache liegt im Test. Im Fall `reads=false` liest das Ziel nicht, darum liegt beim Schließen noch Upload im Empfangspuffer des Proxys. Linux meldet das dem Client auf einem Unix-Socket als `ECONNRESET`, aber erst, wenn dessen Empfangspuffer leer ist. Der Abbruch ersetzt also das Dateiende, nicht die Antwort. `read_to_end` gibt dann den Fehler zurück, obwohl die Antwort schon im Puffer des Tests steht, und `.expect("readable")` wirft sie weg.

Gemessen in einem Linux-Container (aarch64, 2 CPUs, acht parallele Läufe): 6 Fehlschläge in 120 Läufen am unveränderten Test, 7 in 120 am instrumentierten. In allen 7 instrumentierten Fehlschlägen stand die vollständige Antwort da, 306 Bytes mit 105 Bytes Rumpf, dieselbe Länge wie im Gutfall. Ohne parallele Last 0 in 20. Unter macOS 0 in 40, dort tritt der Fehler nicht auf. Warum der Abbruch nur in etwa jedem zwanzigsten Lauf statt des Dateiendes ankommt, ist nicht gemessen.

Der Weg des Agenten läuft über die Brücke des Shims (`daemon/bin/humanitl-shim/src/bridge.rs`). Nach dem Code behandelt `pump` einen Lesefehler wie ein Dateiende: Was gelesen wurde, ist weitergereicht, danach folgt `shutdown(Write)`. Durch die Brücke hindurch gemessen ist das nicht.

### Spezifikation

- Nur der Test ändert sich, der Proxy bleibt. Den Rest des Uploads wegzulesen brächte für den Client nichts und kostete Bandbreite für einen Rumpf, den niemand mehr braucht.
- `ConnectionReset` wird nur bei `reads=false` hingenommen. Bei `reads=true` ist der Rumpf gelesen, dort bleibt jeder Lesefehler rot.
- In beiden Fällen, auch nach sauberem Dateiende, muss die Antwort ganz dastehen: `502` und so viele Rumpf-Bytes, wie `Content-Length` zusagt (`answer_is_whole`). Das ist strenger als vorher.

### Akzeptanzkriterien

- 360 Läufe unter derselben Last wie oben: 0 Fehlschläge.
- Rot ohne Grenze: Mutant ohne `Stall::NoResponseHead` (`reads=true: the proxy gives up on the target`), Mutant ohne `Stall::NotReading` (`reads=false: the proxy gives up on the target`), Mutant ohne `upload_stalled`-Befund (`LIMIT_004 at the flow`).
- Rot ohne Antwort im Fall mit Abbruch: Mutant mit Panik im Handler bei `Stall::NotReading` und Mutant mit falschem `Content-Length` dort, beide mit leerer Antwort an der `502`-Zusicherung. `a_cut_answer_is_not_whole` hält den Maßstab für gekürzte Antworten fest.

Folge-Kandidaten:

- Dieselbe Form haben weitere `read_to_end` mit `.expect` auf eine vorzeitige Antwort: `limits.rs` (413 und 503) und `Proxy::raw_exchange`. Rot wurden sie hier nicht.
- `backlog/sprint-3.md` und der Kommentar an `write_refusal` (`core.rs`) sagen, mit dem Abbruch gehe die `503` verloren. Für den Unix-Socket spricht die Messung hier dagegen; die Begründung sollte nachgeprüft werden.
- `bridge.rs` schließt die TCP-Seite, wenn beide Richtungen fertig sind. Schreibt der Agent dann noch, kann auf TCP ein echtes `RST` entstehen. Ob ihm das die Antwort nimmt, ist nicht geprüft.

## HUM-360 · Website auf Telefonen: Download-Felder passen in die Breite, Pfeile und Zählwerk der Bühne stehen frei

### Kontext

Zwei alte Mängel. Auf der Download-Seite reichte das Feld bei 320 px bis 647 px; `overflow-x: clip` schnitt Knopf und Schritte mitten im Wort ab. Ursache sind Grid-Spalten `1fr` und `auto`, deren Untergrenze der Mindestinhalt ist, also die längste Befehlszeile und die `nowrap`-Pille. Auf der Bühne ragten bei Klasse F die Pfeile aus dem Bild (unterer Rand 657 von 640), und die Konturziffer `cap__index` lag auf `cap__headline` (320 px: x 156 bis 301, y 323 bis 411 gegen Überschrift 369 bis 458). Dazu fehlte der Navigation in Chrome der Glaskörper: Der Minifier behält je Regel nur den letzten der beiden Namen `backdrop-filter` und `-webkit-backdrop-filter`, im Build stand nur die Präfixfassung, die Chrome nicht auswertet.

### Spezifikation

- Download: Alle Spalten von `.dl__head`, `.dl__pick`, `.panel`, `.panel__files` und `.panel__steps` unterhalb der Zweispaltigkeit sind `minmax(0, 1fr)`. Bis 34 rem bricht `.panel__main` zwischen Wort und Dateiangabe um. Befehle behalten ihre Zeilen und rollen im eigenen Kasten, der per Tastatur erreichbar ist; der Kopieren-Knopf steht darüber in eigener Zeile, damit kein Text unter ihm hindurchläuft.
- Bühne, Klasse F: Die Pfeile stehen immer absolut am Boden von `.cap`, die Abstände rechnen in `svh`. Die Regel für kurze Telefone gilt bis 719 px Höhe und verkleinert nur die Schrift. Das Abschneiden des Textes auf drei Zeilen entfällt. Bis 619 px hält `--cap-t` den Platz des längsten Textes frei, die Bühne richtet sich in `stage.ts` danach.
- Zählwerk: In Klasse F steht es klein rechts in der Zeile der Augenbraue, die große Konturziffer entfällt dort. Der Titel `deck__title` wird ausgeblendet, aber nicht aus dem Barrierebaum genommen, wo das Fenster an die Navigation reicht.
- Navigation: `backdrop-filter` steht in der Hauptregel, der Präfix für Safari vor 18 in einem eigenen `@supports not`-Block, den der Minifier nicht zusammenlegt.

### Akzeptanzkriterien

- `npx astro build`, `npm test` und `npm run check-links` sind grün.
- macOS- und Linux-Feld, de und en, 320, 360, 390, 430 und 768 px: Das Feld endet im Viewport, kein Element ragt heraus, kein waagrechter Scroll der Seite; Befehle rollen nur im eigenen Kasten.
- 320x568, 320x640, 360x560, 360x640, 375x667, 390x844, 430x932, de und en, hell und dunkel: Beide Pfeile liegen in allen 7 Modulen im Bild, `elementFromPoint` trifft sie, ein Klick schaltet weiter, der Text endet über den Pfeilen.
- `cap__index` überschneidet in keinem Modul Überschrift oder Augenbraue.
- Der berechnete `backdrop-filter` der gescrollten Navigation ist in Chrome nicht `none`.

Folge-Kandidaten:

- `src/styles/docs.css` hat zwei Regeln mit demselben Minifier-Verlust (`.code__copy` und die Blatt-Leiste); in Chrome fehlt dort die Unschärfe.

## HUM-364 · Proxy: Klartext-HTTP im `CONNECT`-Tunnel, damit `pi` 1.x sein lokales Modell erreicht

`pi` 1.x tunnelt mit undici (`proxyTunnel: true`) auch `http://`-Ziele: `CONNECT 127.0.0.1:8080`, danach Klartext. Der Proxy erwartete im Tunnel immer TLS, die Verbindung starb, `pi` meldete `Connection error`, und kein Flow erschien. Nachgestellt in der macOS-Sandbox mit `curl -p`; die Ursache ist nicht plattformgebunden.

Akzeptanz:

- Beginnt ein Tunnel mit einem Großbuchstaben, wird er als Klartext-HTTP bedient, sonst wie bisher als TLS, auch beim Scheitern des Handschlags.
- Die Anfrage läuft denselben Weg wie jede andere: Regeln, Hold, Aufzeichnung, Sperre für private Adressen.
- `Host` bleibt an das Tunnelziel gebunden; `https://` im Klartext-Tunnel wird abgelehnt.
- Ein `CONNECT` im Klartext-Tunnel behält die SNI-Prüfung seines TLS.
- Der Blick in den Tunnel und der Handschlag teilen sich eine Frist.
- Ein abgelehnter Handschlag erreicht den Client weiter als Alert (`conf_13`).

## HUM-367 · Statusleiste: Daemon-Neustart auch mit laufender Sitzung, ohne `systemctl`-Hinweis

Nach einem Update läuft der alte Daemon weiter. Die Leiste nannte auch auf dem Mac `systemctl`, und der Knopf war gesperrt, solange eine Sitzung lief.

Akzeptanz:

- Die Leiste nennt keinen Befehl mehr.
- Läuft eine Sitzung, fragt der Knopf in der Leiste nach; der Fokus steht auf Abbrechen, `Escape` bricht ab, und nur die zweite Antwort stoppt die Sitzung und startet neu.
- Die Antwort gilt genau der Sandbox, nach der gefragt wurde.
- Ein Daemon, der busy bleibt, wird nie neu gestartet. Start, Stopp, unbekannter Zustand und eine fehlende Kommandozeile ruhen wie bisher.

## HUM-365 · macOS: OpenCode 2 startet in der Sandbox, mit einem eigenen Port je Sitzung

OpenCode 2 spricht mit einem lokalen Server auf Port 49374 oder, mit `--standalone`, auf einem beliebigen. Beides verbietet das Seatbelt-Profil, die Oberfläche blieb bei `Starting background server...` stehen. Unter Linux hat jede Sandbox ihr eigenes Loopback.

Akzeptanz:

- Das Profil erlaubt neben der Tür genau einen zweiten, zufälligen Port der Sitzung; `HUMANITL_AGENT_PORT` nennt ihn.
- Der Port 49374 bleibt gesperrt, damit die Sandbox keinen Dienst des Hosts erreicht.
- Der Adapter startet den Server auf diesem Port mit einem Passwort je Sitzung und beendet ihn mit der Oberfläche; Fassungen vor 2 starten unverändert.
- Der Server bleibt am Proxy, nur die Oberfläche geht an ihm vorbei.
- `single_socket` prüft weiter einen fremden Port und wählt dafür nie den des Agenten.
- `README.md`, `docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen den zweiten Port.

## HUM-368 · macOS: Das Sitzungsverzeichnis samt Zugangsschlüssel verschwindet mit der Sandbox

Unter Linux liegt der Schlüssel des Modells in einem memfd. Unter macOS lag er als Datei im Sitzungsverzeichnis, und das blieb bis zu sieben Tage liegen.

Akzeptanz:

- Endet die Sandbox, ist das Sitzungsverzeichnis nach der Frist weiterer Shells weg, auch wenn niemand mehr ein Handle hält; ein neuer Start derselben Sitzung behält seines.
- Unter Linux ändert sich nichts.
- `docs/SECURITY.md` nennt die Datei auf der Platte und den Fall nach einem Absturz.
