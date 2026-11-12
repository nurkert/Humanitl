# Sprint 7 · Podman als zweites Sandbox-Backend unter Linux

Ziel des Sprints: Unter Linux wählt der Nutzer neben bwrap ein zweites Backend, Podman, je Konfiguration, Konfigurationsprofil, Sitzung (`--backend`) und Oberfläche. Zuerst die gemeinsame Stufe (rootless Podman mit der OCI-Laufzeit `crun`, geteilter Kernel), danach die Kernel-Stufe (Laufzeit `krun`, crun mit libkrun, eigener Gastkernel). Vorgabe bleibt bwrap. Entscheidungen des Inhabers vom 2026-11-11; Entwurf der Entscheidung in [ADR-0020](../docs/adr/0020-podman-sandbox-backend.md), Analyse, offene Messungen (P0 bis P16, K1 bis K8) und Aufwand im Abschnitt „Result (desk part, 2026-11-11)" von HUM-292 in [`sprint-5.md`](sprint-5.md).

Zwei Grenzen gelten für jedes Issue dieses Sprints und sind Akzeptanzkriterium, auch wo das Issue sie nicht eigens nennt:

1. Keine Einstellung schwächt die drei Garantien (`README.md`, `docs/SECURITY.md`, `tests/escape/`), und ein Projekt senkt die Isolationsstufe nie (CONVENTIONS 4.11). Werte, die eine Garantie schwächen würden, haben keinen Schlüssel.
2. Eine unsichere oder nicht erfüllbare Einstellung wird mit `Diagnostic` abgelehnt, beim Laden und beim Start, nie still angewendet; ein gewähltes Backend fällt nie still auf ein anderes zurück.

Leitlinie: Die Namen `podman`, `crun`, `krun`, `libkrun` und `conmon` stehen nur unter `daemon/crates/sandbox/src/podman/` (HUM-145, neuer Abschnitt `[podman]` in `tools/coupling-baseline.toml`). Kein neuer Port, keine Abstraktion über Podman. Jeder Commit, der eine Aussage aus `docs/SECURITY.md` berührt, ändert `docs/SECURITY.md` und `docs/THREAT-MODEL.md` im selben Commit. Vor HUM-433 sind die Messungen P0 bis P15 auf der Referenzmaschine aufgeschrieben, vor HUM-437 auch P17 (Ergebnis in den Abschnitt von HUM-292), vor HUM-443 die Messungen K1 bis K8. Tor: Keine Stufe ist wählbar, weder in der Konfiguration noch über die CLI, die Oberfläche oder `auto`, bevor ihre Einhänge-Prüfung gelandet (HUM-437, HUM-446) und die Escape-Suite gegen sie in der CI grün ist (HUM-435, HUM-445). Bis zur Zulassung (HUM-448 für Podman, HUM-449 für `krun`) lehnt jeder Start sie mit `SANDBOX_048` ab; nur die Escape-Suite und die Tests starten eine nicht zugelassene Stufe, über ein Argument im Code, nie über einen Schlüssel.

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
| `SANDBOX_041` | Container einer früheren Sitzung gefunden und entfernt, oder nicht entfernbar (Warnung) | HUM-433 |
| `SANDBOX_042` | Ein eingehängter Ordner ist nicht der geprüfte (Gerät und Inode) | HUM-437 |
| `SANDBOX_043` | Abbild fehlt lokal oder trägt nicht den erwarteten Digest | HUM-439 |
| `SANDBOX_044` | SELinux erzwingt Etiketten; ein Einhängen verlangte Umetikettieren des Projekts | HUM-433 |
| `SANDBOX_045` | Laufzeit `krun` fehlt oder `/dev/kvm` ist nicht les- und schreibbar | HUM-444 |
| `SANDBOX_046` | Tür der Kernel-Stufe nicht ausdrückbar: Weiterleitung an oder vsock-Abbildung fehlt | HUM-444 |
| `SANDBOX_047` | Host-Prüfung des VMM gescheitert (mehr als `lo`, unerklärter Deskriptor) | HUM-444 |
| `SANDBOX_048` | Stufe noch nicht zugelassen; wird von HUM-448 (Podman) und HUM-449 (`krun`) freigegeben | HUM-431 |
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
| HUM-443 | Shim: Steuerkanal als Strom, Terminal und weitere Shells im Gast | L | HUM-442 |
| HUM-444 | Podman-Adapter, Kernel-Stufe: `krun`, vsock-Tür, Host-Prüfung des VMM | L | HUM-443 |
| HUM-445 | Escape-Suite der Kernel-Stufe: Proben mit Gast-Root, CI-Job mit KVM | M | HUM-444 |
| HUM-446 | Kernel-Stufe: Projektordner und Wegwerf-Schicht über virtiofs | L | HUM-445 |
| HUM-449 | Kernel-Stufe zulassen: `krun` wird wählbar | S | HUM-445, HUM-446 |
| HUM-447 | Doku der Kernel-Stufe, Oberfläche schaltet `krun` frei | S | HUM-449 |
| HUM-451 | Integrationstest: ein Daemon, der die Shell-Tür nicht mehr liest, hält weder Lebensleine noch Agent auf | S | HUM-430 |
| HUM-450 | `humanitl sandbox argv` hält zurückgehaltene Werte auch in der CLI zurück | S | HUM-429 |

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
4. Was nicht zugelassen ist (`SANDBOX_048`), zeigt die Oberfläche nicht als Wahl; `podman` erscheint nach HUM-448, `krun` erst mit HUM-447.
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
Die Kernel-Stufe hängt daran, ob crun mit libkrun die Tür so ausdrücken kann, wie die Garantien es verlangen: kein Netzgerät, keine transparente Socket-Weiterleitung, genau ein vsock-Port auf den Unix-Socket der Sitzung, kein `vhost-vsock`. Messungen K1 bis K8 aus dem HUM-292-Ergebnis. Zeitrahmen zwei Tage; Prototyp außerhalb von `main`.

### Spezifikation
1. K1 bis K8 messen und mit Befehl und Ausgabe in den Abschnitt von HUM-292 eintragen.
2. Antwort auf: Lässt sich Weiterleitung ganz abschalten und genau ein Port abbilden (K1, K2)? Welche Deskriptoren und Kanäle hat der VMM (K3)? Reichen Inode-Nummern über virtiofs durch (K5)? Gehen zusätzliche Host-Ordner in den Gast (K6)? Wie kommt libkrun in die CI (K7, K8)?
3. Entscheidung im Nachtrag von ADR-0020: crun-Weg oder Rückfall „libkrun direkt aus dem Adapter, im bwrap-Gefängnis"; bei Rückfall werden HUM-444 und HUM-446 vor dem Start neu geschnitten.

### Tests
- Keine im Repository; Belege sind die Ausgaben.

### Akzeptanzkriterien
1. Jede Frage aus Punkt 2 hat einen Beleg oder ist mit Grund offen.
2. Der Inhaber entscheidet daraus über den Weg der Kernel-Stufe.

### Fallstricke
- libkrun ist in Debian nicht paketiert; der Bau steht im Spike-Bericht mit festen Versionen, damit er wiederholbar ist.

## HUM-443 · Shim: Steuerkanal als Strom, Terminal und weitere Shells im Gast

### Kontext
In einer VM überquert kein Deskriptor die Grenze: Das PTY kann nicht vom Daemon kommen, `SCM_RIGHTS` für weitere Shells (HUM-299) geht nicht, ein Host-Signal erreicht keinen Gastprozess. Der Shim braucht einen zweiten, erklärten Kanal, der als Strom läuft.

### Spezifikation
1. Ein gerahmtes Protokoll auf einem Strom-Socket (im Gast `AF_VSOCK`, in Tests ein Unix-Socket-Paar): Nachrichten für Terminal-Bytes je Kanal, Größe, Shell öffnen und schließen, Exit, Interrupt, Lebenszeichen; feste Höchstlängen, unbekannte Kennungen verworfen, wie die Tür heute.
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

## HUM-444 · Podman-Adapter, Kernel-Stufe: `krun`, vsock-Tür, Host-Prüfung des VMM

### Kontext
Nach HUM-442 ist bekannt, wie die Tür ausgedrückt wird. Die Garantien müssen auch mit Root im Gast halten; die Prüfungen im Gast sind dann nur Tiefenverteidigung.

### Spezifikation
1. `sandbox.podman.runtime = "krun"`: `isolation()` ist `Kernel`; `detect` prüft Laufzeit und `/dev/kvm` (`SANDBOX_045`).
2. Plan wie HUM-433 plus: Weiterleitung aus, genau ein vsock-Port auf den Proxy-Socket der Sitzung, ein zweiter Port für den Steuerkanal (HUM-443), kein `vhost-vsock`, vCPU und Arbeitsspeicher aus `sandbox.podman`; lässt sich eins davon nicht ausdrücken: `SANDBOX_046`, kein Start.
3. Im Gast lauscht der Shim auf `127.0.0.1:3128` und reicht an den vsock-Port weiter; für den Agenten ändert sich nichts.
4. `isolation_check` liest die `CHECK`-Zeilen aus dem Gast **und** prüft auf dem Host den VMM-Prozess: Netz-Namensraum nur `lo`, offene Sockets nur die erklärten (Proxy, Steuerkanal), keine `vhost-vsock`-Deskriptoren. Scheitert die Host-Prüfung: `SANDBOX_047`, Sitzung gestoppt. Probestart in `doctor`: `DOCTOR_015`.
5. `docs/SECURITY.md` Abschnitt „Linux: Podman mit eigenem Kernel" formuliert die drei Garantien für die Kernel-Stufe; `docs/THREAT-MODEL.md` K-Einträge für VMM, vsock-Tür und Gastkernel.

### Tests
- Unit: die Annotationen bzw. Optionen enthalten genau einen Tür-Port und keine Weiterleitung; eine Verbotsliste prüft das.
- Integration (übersprungen ohne KVM und krun): Sitzung startet, Terminal funktioniert, Host-Prüfung grün.
- Mutationsbeweis: Weiterleitung eingeschaltet färbt die Host-Prüfung oder `no_interfaces` rot; ein zusätzlicher vsock-Port färbt `single_socket` rot.

### Akzeptanzkriterien
1. Die drei Garantien gelten mit Root im Gast (belegt in HUM-445).
2. Fehlt KVM oder krun, startet nichts, und der Code nennt den Grund; kein Rückfall auf `crun`.
3. Für Nutzer bleibt `krun` mit `SANDBOX_048` gesperrt, bis HUM-449 landet; gestartet wird nur über `detect_unadmitted`.

### Fallstricke
- `podman exec` gibt es unter krun nicht; nichts im Adapter darf es brauchen.
- Die PID aus `podman inspect` ist der VMM; ein Signal dorthin beendet die VM, es unterbricht nicht den Agenten (Interrupt über HUM-443).

## HUM-445 · Escape-Suite der Kernel-Stufe: Proben mit Gast-Root, CI-Job mit KVM

### Kontext
Die Kernel-Stufe ist erst zulässig, wenn die ganze Suite gegen sie grün ist und Proben belegen, dass Root im Gast nichts außer der Tür erreicht.

### Spezifikation
1. `HUMANITL_ESCAPE_BACKEND=podman-krun` über `detect_unadmitted`; alle Proben laufen, auch die Mounttabellen-Probe aus HUM-437.
2. Neue Proben: aus dem Gast als Root und ohne seccomp (nur im Testprofil, nie wählbar außerhalb der Suite) andere vsock-Ports und CIDs nicht erreichbar; `ip link` zeigt nur `lo`; ein `connect` nach außen scheitert; der Netz-Namensraum des VMM hat nur `lo` (Probe auf dem Host).
3. CI-Job `escape-tests-krun` auf `ubuntu-24.04` mit KVM-Freigabe (K7) und libkrun nach dem Weg aus K8; Pfadregeln nach HUM-424.

### Tests
- Die Suite; Mutationsbeweise je neuer Probe (zusätzliches Netzgerät, zweiter vsock-Port, Weiterleitung an), im Commit-Body mit Lauf-ID.

### Akzeptanzkriterien
1. `escape-tests-krun` grün in der CI, keine Probe übersprungen, die bwrap grün hat.
2. Das Testprofil ohne seccomp ist nur von der Suite ladbar (`SocketFloor`-Muster: Argument im Code, kein Schlüssel).

### Fallstricke
- Der Bau von libkrunfw dauert; der Cache-Schlüssel enthält die festen Versionen, sonst baut jeder Lauf neu.

## HUM-446 · Kernel-Stufe: Projektordner und Wegwerf-Schicht über virtiofs

### Kontext
Unter krun kommt `/work` über virtiofs in den Gast. Gerätenummern unterscheiden sich, Inodes nach K5. Die Wegwerf-Schicht braucht dieselbe Semantik wie HUM-350, mit dem Inhalt der Schicht als Daten des Angreifers.

### Spezifikation
1. Pinning nach K5: gleiche Inodes, dann Vergleich wie HUM-437 im Gast plus Host-Prüfung der vom VMM geöffneten Verzeichnisse; sonst nur Host-Prüfung, und `docs/SECURITY.md` sagt das.
2. Wegwerf: obere Schicht als Host-Verzeichnis über virtiofs oder nach K6 eine andere Form; der Daemon liest sie erst nach dem Ende der VM, mit dem vorhandenen Scan; die Masken gelten wie überall.
3. `sandbox.mode`-Regeln aus HUM-357 unverändert (`SANDBOX_030`, `SANDBOX_033`).

### Tests
- Integration (übersprungen ohne KVM): Tauschtest aus HUM-437 gegen krun; ESC-5 Wegwerf-Lauf gegen krun.
- Mutationsbeweis: Host-Prüfung der Ordner entfernt färbt den Tauschtest rot.

### Akzeptanzkriterien
1. Kein Agent startet in einem nicht geprüften Ordner; ein Wegwerf-Lauf schreibt nie ins Projekt.
2. `docs/THREAT-MODEL.md` K-Eintrag für virtiofs.

### Fallstricke
- Mit Root im Gast ist jede Datei der Schicht vom Angreifer geschrieben, auch Whiteouts und Rechte; der Scan bleibt so streng wie unter bwrap.

## HUM-449 · Kernel-Stufe zulassen: `krun` wird wählbar

### Kontext
Gegenstück zu HUM-448 für die Kernel-Stufe.

### Spezifikation
1. Voraussetzung, im Commit-Body mit Lauf-ID belegt: `escape-tests-krun` grün auf `main` mit HUM-446 (Ordner und Wegwerf über virtiofs) und den Proben mit Gast-Root aus HUM-445.
2. `ADMITTED_BACKENDS` bekommt `podman` mit der Laufzeit `krun`.
3. `docs/SECURITY.md` Abschnitt „Linux: Podman mit eigenem Kernel" verliert den Hinweis „noch nicht zugelassen"; `docs/THREAT-MODEL.md` setzt die K-Einträge für VMM, vsock-Tür, Steuerkanal und virtiofs auf `mitigated` mit ihren Proben.

### Tests
- Der Zulassungstest erwartet `krun` in der Liste; `sandbox.min_isolation = "kernel"` mit `runtime = "krun"` startet (mit Attrappe).

### Akzeptanzkriterien
1. `humanitl run --backend podman --podman-runtime krun` startet eine Sitzung mit eigenem Kernel.
2. Der Commit ändert keine Zeile außerhalb der Zulassungsliste, ihrer Tests und der zwei Sicherheitsdokumente.

### Fallstricke
- Ohne libkrun auf dem Rechner bleibt `krun` mit `SANDBOX_045` nicht verfügbar; das ist kein Fehler der Zulassung.

## HUM-447 · Doku der Kernel-Stufe, Oberfläche schaltet `krun` frei

### Kontext
Abschluss der Kernel-Stufe, nach der Zulassung (HUM-449).

### Spezifikation
1. `README.md` nennt die Kernel-Stufe; `docs/CONFIG.md`, `docs/cli.md` nachgezogen; ADR-0020 Nachtrag „Kernel-Stufe zugelassen" mit Lauf-ID.
2. Oberfläche (HUM-440) zeigt `krun` als Laufzeit, mit Satz zur Stufe und Voraussetzungen aus dem RPC.
3. `.deb`: Laufzeit `krun` nur als `Suggests`, solange Debian libkrun nicht paketiert.

### Tests
- Widget: `krun` wählbar nur mit `available = true`; Golden aktualisiert.
- `lint-docs.sh`, `check-doc-links.py`.

### Akzeptanzkriterien
1. Jede Aussage zur Kernel-Stufe ist durch `escape-tests-krun` und `docs/SECURITY.md` gedeckt.

### Fallstricke
- Das Tor aus dem Kopf dieses Sprints gilt: `krun` erscheint in der Oberfläche nicht vor HUM-449.

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
