# Sprint 6 · macOS (M12)

Ziel des Sprints: Humanitl läuft auf macOS (Apple Silicon und Intel) so sauber wie auf Linux, und jedes Release trägt eine funktionierende Mac-App. Wunsch des Nutzers vom 2026-10-29. Entscheidungen des Nutzers vom selben Tag: Die Sandbox auf macOS ist zuerst Seatbelt (`sandbox-exec` mit einem erzeugten SBPL-Profil), ein Backend mit Linux-VM folgt später als eigenes Issue (HUM-318). Die App wird ohne Apple Developer ID nur ad-hoc signiert. Die Arbeit landet auf dem Feature-Branch `macos-support`, ein Commit je Issue, und geht erst nach `main`, wenn Linux und macOS grün sind; das weicht bewusst von `CLAUDE.md` Schritt 5 ab.

Leitlinie: Linux bleibt, wie es ist. Jede Änderung an gemeinsamem Code ist entweder plattformneutral oder hinter `#[cfg(target_os = ...)]` beziehungsweise `Platform.isMacOS`, und die Linux-CI bleibt die Referenz.

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-312 | Daemon, CLI und Tests bauen auf macOS | M | — |
| HUM-313 | Seatbelt-Backend: die Sandbox auf macOS | L | HUM-312 |
| HUM-314 | Der Daemon läuft auf macOS als launchd-Agent | M | HUM-312 |
| HUM-315 | Die App läuft auf macOS | M | HUM-312 |
| HUM-316 | Jedes Release trägt eine Mac-App | M | HUM-313, HUM-314, HUM-315 |
| HUM-317 | Sicherheitsaussage und Bedrohungsmodell für macOS | S | HUM-313 |
| HUM-318 | Linux-VM als zweites Backend auf macOS (später) | L | HUM-313 |
| HUM-319 | Tray und Benachrichtigungen auf macOS (später) | M | HUM-315 |
| HUM-320 | Vorschau und Mounttabelle zeigen das Seatbelt-Profil | S | HUM-313 |
| HUM-321 | Der Proxy nimmt unter macOS nur den Shim der laufenden Sitzung an | M | HUM-313 |
| HUM-322 | Kleinere Folgen aus den Reviews von M12 | S | HUM-312 bis HUM-317 |

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

## HUM-313 · Seatbelt-Backend: die Sandbox auf macOS

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

## HUM-314 · Der Daemon läuft auf macOS als launchd-Agent

### Spezifikation
1. Laufzeitverzeichnis auf macOS: `confstr(_CS_DARWIN_USER_TEMP_DIR)/humanitl`, für Terminal und launchd dieselbe Stelle und kurz genug für `sun_path`.
2. `humanitl daemon install|uninstall|restart|status|logs` auf macOS über eine LaunchAgent-Datei `~/Library/LaunchAgents/dev.humanitl.humanitld.plist` (`RunAtLoad`, `KeepAlive` bei Fehlschlag) und `launchctl bootstrap|bootout|kickstart|print`; Protokoll nach `~/Library/Logs/Humanitl/humanitld.log`.
3. Diagnosen nennen auf macOS `launchctl` statt `systemctl`.
4. `humanitl doctor` prüft auf macOS `sandbox-exec`, das Laufzeitverzeichnis, den LaunchAgent und lässt bwrap, user namespaces, seccomp, tray und renderer weg.

## HUM-315 · Die App läuft auf macOS

### Spezifikation
1. `app/macos` mit Titel, Größe 1280×720, ohne doppelte Titelleiste, App Sandbox aus (die App muss Socket, Token und die CLI neben sich erreichen).
2. Die Prüfung privater Pfade nutzt auf macOS `lstat` statt `statx`.
3. Fensterport auf macOS aktiv; die eigenen Fensterknöpfe entfallen dort zugunsten der Ampel.
4. Tastenkürzel nehmen auf macOS Cmd statt Ctrl.
5. Neustart des Daemons und Einrichtung gehen über die CLI (`humanitl daemon restart|install`), nicht über `systemctl`. Die CLI liegt in `Humanitl.app/Contents/MacOS/bin`.
6. Dateien öffnen mit `open`; Zustandsdateien unter `~/Library/Application Support/Humanitl`.

## HUM-316 · Jedes Release trägt eine Mac-App

### Spezifikation
1. Der Release-Workflow baut auf `macos-15` die Binaries für `aarch64-apple-darwin` und `x86_64-apple-darwin`, verbindet sie mit `lipo`, baut die App, legt `humanitld`, `humanitl` und `humanitl-shim` in `Contents/MacOS/bin`, signiert ad-hoc (`codesign --force --deep -s -`) und packt `Humanitl-<version>-macos.dmg` und `.zip`.
2. Ein Prüfjob auf einem frischen macOS-Runner entpackt das Archiv, prüft Signatur und Architekturen, startet den Daemon aus dem Bündel und fährt `humanitl sandbox check`.
3. Das Release führt beide Dateien und ihre Prüfsummen in `SHA256SUMS`; die Release-Notes nennen den Quarantäne-Schritt.

## HUM-317 · Sicherheitsaussage und Bedrohungsmodell für macOS

### Spezifikation
`docs/SECURITY.md` und `docs/THREAT-MODEL.md` nennen die Garantien je Backend, die Abweichungen von Linux (Host-geteilter Loopback, kein PID-Namensraum, Leserechte nach Pfaden statt Mounts, Seatbelt ist undokumentiert und als veraltet markiert) und ihre Folgen. ADR-0019 hält die Wahl von Seatbelt fest. `README.md` und `docs/INSTALL.md` nennen macOS.

## HUM-318 · Linux-VM als zweites Backend auf macOS (später)

Entscheidung des Nutzers vom 2026-10-29: nach Seatbelt. Eine Linux-VM über `Virtualization.framework` mit dem bestehenden bwrap-Backend im Gast, die Tür als vsock-Port. Damit gelten auf macOS dieselben drei Garantien wie auf Linux. Umfang, Gastabbild und Startzeit legt ein Spike fest.

## HUM-319 · Tray und Benachrichtigungen auf macOS (später)

Unter Linux laufen Tray und Benachrichtigungen über D-Bus (StatusNotifierItem, `org.freedesktop.Notifications`). Auf macOS braucht es ein `NSStatusItem` und `UNUserNotificationCenter` mit den Aktionen Erlauben, Blockieren, Anzeigen. Bis dahin zeigt die App auf macOS den Hinweis „kein Tray“ wie auf einem Linux ohne Tray.

## HUM-320 · Vorschau und Mounttabelle zeigen das Seatbelt-Profil

### Kontext
Aus HUM-313: Der Plan-RPC baut seine Vorschau (`argv_preview`, Mounttabelle der Oberfläche, `humanitl sandbox argv`) aus `SandboxProfile::to_bwrap_args` und liest die Mounts aus deren Flags (`ipc/src/sandbox.rs`, `Prepared::argv`, `mounts_of`). Unter macOS startet aber `sandbox-exec` mit einem SBPL-Profil; die Vorschau zeigt dort eine bwrap-Zeile, die nie läuft.

### Spezifikation
1. Unter macOS zeigt die Vorschau die Kommandozeile von `sandbox-exec` und den Text des Profils, ohne Seiteneffekt (kein Sitzungsverzeichnis, kein Port).
2. Die Mounttabelle kommt aus derselben Beschreibung wie das Profil (`SeatbeltPolicy`): Projekt mit Modus, Sitzungsverzeichnis, zusätzliche Ordner, Masken.
3. Der Schalter in der Oberfläche heißt neutral („Exakte Sandbox-Kommandozeile").

## HUM-321 · Der Proxy nimmt unter macOS nur den Shim der laufenden Sitzung an

### Kontext
Aus der Review von HUM-317 (K-19, K-20): Unter macOS fehlen PID-Namensraum und seccomp. Ein Prozess, der sich mit `setsid` aus der Sitzung des Agenten löst, überlebt das Ende der Sitzung, und das Seatbelt-Profil erlaubt ihm weiter den Proxy-Socket des Daemons, der für den ganzen Lauf des Daemons gilt. Er kann Anfragen stellen, solange der Daemon läuft; `allow`-Regeln und der Passthrough gelten. Dazu erreicht jeder lokale Prozess jedes Kontos den Loopback-Port der Tür.

### Spezifikation
1. Der Proxy liest unter macOS bei jeder Verbindung auf dem Proxy-Socket die PID der Gegenseite (`getsockopt(LOCAL_PEERPID)`) und nimmt nur Verbindungen der Shims laufender Sitzungen an; der Daemon trägt die PID beim Start ein und beim Ende aus. Abgewiesene Verbindungen zählt und meldet er wie abgewiesene Sockets (HUM-138).
2. Die Tür des Shims prüft die Gegenseite auf Loopback ebenso über ihre PID und nimmt nur Prozesse derselben Sitzung oder einer weiteren Shell der Sitzung an.
3. Die Escape-Suite unter macOS bekommt Proben: ein mit `setsid` gelöster Prozess erreicht den Proxy nach dem Ende der Sitzung nicht mehr, ein Prozess außerhalb der Sandbox erreicht die Tür nicht.
4. `docs/SECURITY.md` Abschnitt 12, K-19 und K-20 werden entsprechend eingeschränkt.

## HUM-322 · Kleinere Folgen aus den Reviews von M12

1. Die Titel im Register der Codes nennen unter macOS systemd (`DAEMON_008`, `DOCTOR_005`); sie werden plattformneutral oder je Plattform.
2. Kopierbare Befehle nennen unter macOS `humanitl` ohne Pfad, der Befehl liegt aber nur im Bündel; sie nennen den Pfad aus `installServiceCandidate` oder die Anleitung zum Verlinken.
3. Die Ampel des Fensters sitzt etwa 6 px über der Mitte der 40 px hohen Kopfleiste; im Vollbild bleibt die Lücke von 72 px. `MainFlutterWindow` richtet die Knöpfe aus, die Kopfleiste kennt den Vollbildzustand.
4. Hinweise auf Tastenkürzel zeigen unter macOS „Ctrl"; sie zeigen ⌘, die `Ctrl`-Form bleibt wirksam.
5. Eine App aus Downloads startet unter macOS aus einer vorübergehenden Kopie (App Translocation); `daemon install` erkennt das und verweigert mit Hinweis auf `/Applications`.
6. Die Prüfung neuer macOS-Versionen: eine CI-Matrix über `macos-14`, `macos-15` und `macos-latest` für die Escape-Suite.
7. Ein Probe für unprivilegiertes ICMP (`SOCK_DGRAM`, `IPPROTO_ICMP`) im Shim und in der Escape-Suite.
