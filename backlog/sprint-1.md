# Sprint 1 · Sealed Box (M1)

Ziel des Sprints: Die Sandbox ist nachweislich dicht, der Proxy hält einen `curl`-Request aus der Sandbox an, und ein erstes Flutter-UI zeigt ihn gegen den Fake-Daemon. Am Ende läuft `humanitl sandbox run -- curl https://example.com` und der Request erscheint als `Held` im gRPC-Stream.

Voraussetzungen aus Sprint 0: HUM-001 (Monorepo), HUM-002 (CI), HUM-003 (Proto v1), HUM-004 (core-types), HUM-005 (Fake-Daemon), HUM-006 (Escape-Harness), HUM-010 (Profil-Format), HUM-062 (config), HUM-063 (Diagnostic).

| ID | Titel | Größe | Abhängigkeiten |
|---|---|---|---|
| HUM-011 | bwrap-Launcher | M | HUM-004, HUM-010, HUM-063 |
| HUM-012 | humanitl-shim | M | HUM-011 |
| HUM-013 | Proxy-Socket-Bind | S | HUM-011, HUM-012 |
| HUM-014 | CA-Verwaltung und Env-Kit | M | HUM-011 |
| HUM-015 | MITM-Proxy-Kern | L | HUM-004, HUM-014 |
| HUM-016 | Hold-Queue | M | HUM-004, HUM-015 |
| HUM-017 | Konformitäts-Matrix | M | HUM-015, HUM-016 |
| HUM-018 | gRPC-Server Grundgerüst | M | HUM-003, HUM-016 |
| HUM-064 | CLI-Grundgerüst | M | HUM-062, HUM-018 |
| HUM-019 | Flutter-Shell | M | HUM-003, HUM-005, HUM-008 |
| HUM-020 | Intercept-Screen v1 | L | HUM-019 |
| HUM-021 | Demo-Skript M1 | S | alle oben |
| HUM-108 | `PROXY_007` wird nie ausgegeben | S | HUM-017, HUM-045 |
| HUM-109 | `HoldQueue::extend` ist von nichts erreichbar | M | HUM-016, HUM-018, HUM-064 |
| HUM-110 | WebSocket-Upgrade kommt nie zustande | L | HUM-015, HUM-017, HUM-026 |
| HUM-111 | Die Sicherheitsdokumente behaupten, was der Code nicht tut | M | HUM-011, HUM-013, HUM-017, HUM-024 |
| HUM-112 | hudsucker ist keine Abhängigkeit mehr, ADR-0001 sagt das Gegenteil | S | HUM-009, HUM-015 |
| HUM-113 | `escape-launch` ist verwaist | S | HUM-064 |

Leseanweisung für die Umsetzung: `BACKLOG.md` Abschnitte 2 bis 6, dann `backlog/CONVENTIONS.md`, dann das Issue. Jede Signatur in diesem Dokument ist verbindlich; Abweichungen werden im PR begründet.

---

> **Versionsbefund 2026-09-02** (gemessen, nicht recherchiert): `hudsucker` ist aktuell **0.24.1**, nicht 0.25 wie in BACKLOG.md ADR-001 angenommen. `tonic` ist 0.14.5 und hat den prost-Teil abgespalten: Laufzeit `tonic` + `tonic-prost`, Bauzeit `tonic-prost-build`. `protox` 0.9.1 ersetzt `protoc`. Weitere gepinnte Versionen stehen in `daemon/Cargo.toml` unter `[workspace.dependencies]`; Member-Crates referenzieren sie mit `dep.workspace = true` und tragen nie eine eigene Version. Auf dieser Maschine fehlen `rustfmt`, `clippy`, `rustup`, `protoc` und `buf`; `make check` überspringt Format und Lint mit Hinweis, CI setzt `STRICT=1`.
>
> **Review-Korrekturen 2026-09-02** (gelten vor dem Text; Details CONVENTIONS 4.10): HUM-012: `socket()` nur `AF_INET`/`AF_INET6` mit `SOCK_STREAM` (arg1 & 0xff), Arch-Mismatch `KillProcess`, x32-Bit-Syscalls `EPERM` per BPF-Präludium, kein „falls"; bwrap-Argv `--cap-drop ALL` (HUM-011). HUM-015: Schritt 0 ist ein Spike, ob hudsucker einen generischen Accept-Stream annimmt, sonst Fork der Accept-Schleife auf `UnixListener`; der Loopback-TCP-Port auf dem Host entfällt in beiden Fällen. Kein `GaiResolver`: DNS nur über den `Resolver`-Port nach Allow, Verbindung über `Egress::connect(authority, Some(ip))`. `Expect: 100-continue` wird sofort beantwortet, der Body landet im Hold-Puffer, nichts geht vor der Entscheidung zum Upstream. HUM-016: `limits.hold_max_bytes` (256 MiB) und `limits.hold_max_flows` (200) als atomare Zähler, Überschreiten ⇒ `503`. HUM-017: gRPC-Zeile ist in M1 „erwartet: fehlschlägt mit `PROXY_007`", ALPN bietet dem Client nur `http/1.1`. Statuscodes: `403` Policy, `413` Body-Cap, `504` Timeout, `502` Upstream, `503` Budget.
>
> **Abgleich 2026-09-02**: Der Shim startet eine *Liste* von Bridges aus `[network].bridges` des Profils (MVP: genau eine, Richtung `in`, Proxy); Richtung `out` (Host verbindet in die Sandbox, später Browser-CDP, ADR-016) ist als Enum-Variante vorgesehen, aber nicht gebaut. seccomp-Familien kommen aus `[seccomp].allow_families` (Default `AF_INET`, `AF_INET6`). **Nachtrag 2026-09-03 (Review)**: Beide Listen sind keine freien Angaben mehr, sondern Böden in beide Richtungen — `network.bridges` ist genau die Proxy-Bridge, `allow_families` genau `AF_INET`, `AF_INET6` und `allow_types` genau `SOCK_STREAM`; siehe `CONVENTIONS.md` 4.12. Der Proxy öffnet Upstream-Verbindungen ausschließlich über den Port `Egress` (`Egress::Direct` im MVP, ADR-017); `TcpStream::connect` außerhalb `proxy/src/egress/` bricht `tools/check-deps.sh`. Escape-Test-Dateien heißen `esc-1-sockets.sh`, `esc-2-mounts.sh`, `esc-3-egress.sh`.

> **Bestandsaufnahme 2026-09-05** (jedes Akzeptanzkriterium von elf Issues dieses Sprints gegen den Code geprüft; HUM-015 war nicht Teil der Aufnahme und ist unangetastet): 24 Kriterien erfüllt und abgehakt, 14 teilweise, 2 nicht erfüllt (HUM-013 Sitzungs-Sockets, HUM-021 DNS-Zähler), 2 überholt formuliert (HUM-013 `find -type s`, HUM-016 „403 bei Timeout"). Nach Kriterien vollständig: HUM-012, HUM-014. Neu angelegt aus der Aufnahme: HUM-108 (`PROXY_007` wird nie ausgegeben), HUM-109 (`HoldQueue::extend` ist von nichts erreichbar), HUM-110 (WebSocket-Upgrade kommt nie zustande), HUM-111 (die Sicherheitsdokumente behaupten, was der Code nicht tut), HUM-112 (hudsucker ist keine Abhängigkeit mehr, ADR-0001 sagt das Gegenteil), HUM-113 (`escape-launch` ist verwaist). Schon getrackt und nur am Kästchen verwiesen: HUM-097 (Bildschirm-Hälfte des Laufs), HUM-087 (`https://` durch die Sandbox), HUM-101 (`ui.notifications`, `experimental.ws_hold`), HUM-098 (Reduzierer), HUM-058 (Karten). Bewusst nicht als Issue: `SessionSocket::create` ohne Aufrufer (CONVENTIONS 4.12 verschiebt die zweite Sitzung nach Sprint 3, dort gehört er hin), die Form der Startzeile im Log (niemand liest sie), grpcurl ohne Reflection (`-protoset proto/descriptor.binpb` genügt), die Pane-Breiten über den Neustart (Halbsatz an HUM-020, wartet auf den Einstellungsspeicher), die 2-s- und 300-ms-Budgets ohne Messung (HUM-097 misst den Lauf) und die Nummerierungskollision 4.17 in CONVENTIONS (eine Korrektur, kein Issue).

## HUM-011 · bwrap-Launcher

> **Register-Abgleich 2026-09-02**: Die SANDBOX-Codes in dieser Spezifikation sind durch das Register in `daemon/crates/core-types/src/diagnostics/codes.rs` überholt. Verbindlich: verbotener Mount ⇒ `SANDBOX_006` (nicht 004), Projektordner nicht beschreibbar ⇒ `SANDBOX_005` (nicht 006), Bridge-Richtung `out` ⇒ `SANDBOX_007`, Proxy-Socket-Datei oder Shim-Binary fehlt ⇒ `SANDBOX_011` (Platzhalter nicht anlegbar) mit dem Pfad im Befund, `work_src` fehlt ⇒ `SANDBOX_005` mit eigener `why`-Zeile. Die Mount-Verbotsliste wird nicht hier, sondern in `humanitl-sandbox::MountPolicy` (HUM-010, `load_validated`) geprüft; der Launcher ruft sie auf. Zusätzlich aus HUM-010: maskierte Dateien nicht per `--ro-bind /dev/null`, sondern per `--ro-bind-data <FD>` aus einem leeren memfd (der /dev/null-Bind liegt auf einem nodev-Mount und liefert EACCES); bwrap mit gesäuberter Umgebung starten, weil `/proc/1/environ` sonst die Host-Umgebung zeigt (ESC-2-Befund); `--cap-drop ALL` kommt aus dem Profil-Renderer, `--disable-userns` ab bwrap 0.6 zusätzlich.

Sprint: 1 · Größe: M · Abhängigkeiten: HUM-004, HUM-010, HUM-063 · Blockiert: HUM-012, HUM-013, HUM-014

### Kontext
Setzt ADR-002 um. Die Sandbox ist ein bubblewrap-Aufruf mit leerem Netzwerk-Namespace. Die gesamte Isolations-Policy ist eine einzige Argument-Liste, die das UI später wörtlich anzeigt (Isolation-Panel, HUM-041). Dieses Issue liefert den `SandboxBackend`-Trait und die bwrap-Implementierung, die aus einem `SandboxProfile` einen `LaunchPlan` baut und ihn startet.

### Ziel
`humanitl_sandbox::BwrapBackend` erzeugt aus `profiles/sandbox/default.toml` plus Session-Kontext (Projektpfad, Modus ro/rw, Proxy-Socket-Pfad, CA-Pfad, Agent-Kommando) eine vollständige, deterministische bwrap-Argv-Liste, startet den Prozess und liefert einen `SandboxHandle` mit PID, Exit-Future und dem Argv-String. Innerhalb der Sandbox existiert nur `lo`, kein `/etc/resolv.conf`, kein Host-Home, nur die im Profil erlaubten Mounts.

### Nicht-Ziel
Kein seccomp (HUM-012), kein Proxy-Socket-Bind (HUM-013, hier nur der Mount-Slot), kein Env-Kit-Inhalt (HUM-014, hier nur der Mechanismus `--setenv`), keine `/work`-Härtung über die Profil-Primitive hinaus (HUM-043), kein Docker-Backend (M6).

### Betroffene Pfade
- `daemon/crates/sandbox/Cargo.toml` (neu)
- `daemon/crates/sandbox/src/lib.rs` (neu): Trait, Re-Exports
- `daemon/crates/sandbox/src/profile.rs` (neu, übernimmt Parser aus HUM-010)
- `daemon/crates/sandbox/src/plan.rs` (neu): `LaunchPlan`, Argv-Builder
- `daemon/crates/sandbox/src/bwrap.rs` (neu): `BwrapBackend`
- `daemon/crates/sandbox/src/handle.rs` (neu): `SandboxHandle`
- `daemon/crates/sandbox/src/diag.rs` (neu): Diagnostic-Codes `SANDBOX_001..009`
- `daemon/crates/sandbox/tests/argv.rs` (neu): Snapshot-Tests
- `profiles/sandbox/default.toml`, `profiles/sandbox/test.toml` (neu)

### Spezifikation

Trait und Typen exakt wie CONVENTIONS 3.4. Ergänzend:

```rust
pub struct SessionContext {
    pub session: SessionId,
    pub work_src: PathBuf,             // absoluter, kanonisierter Host-Pfad
    pub work_mode: WorkMode,           // Ro | Rw
    pub proxy_socket_src: PathBuf,     // Host-Pfad der Socket-Datei (HUM-013)
    pub ca_cert_src: PathBuf,          // Host-Pfad ca.crt (HUM-014)
    pub ca_bundle_src: PathBuf,        // Host-Pfad des generierten ca-certificates.crt (HUM-014)
    pub shim_src: PathBuf,             // Host-Pfad des humanitl-shim-Binaries
    pub agent_argv: Vec<OsString>,     // z. B. ["opencode"] oder ["curl", "https://example.com"]
    pub extra_env: Vec<(String, String)>,
    pub passwd: Vec<u8>,               // generierte /etc/passwd (eine Zeile für UID)
    pub group: Vec<u8>,
}

pub struct SandboxHandle {
    pub id: SandboxId,
    pub pid: u32,                      // PID des bwrap-Prozesses auf dem Host
    pub argv_display: String,          // shell-escaped, für UI und `humanitl sandbox argv`
    child: tokio::process::Child,
}
impl SandboxHandle {
    pub async fn wait(&mut self) -> Result<ExitStatus, Diagnostic>;
    pub fn kill(&mut self) -> Result<(), Diagnostic>;   // SIGTERM, nach 5 s SIGKILL
}
```

Verbindliche Argv-Reihenfolge (jede Zeile ein Argument oder Paar; `<>` = aus Kontext/Profil):

| Argument | Begründung |
|---|---|
| `bwrap` | Pfad aus `which`, Version ≥ 0.8 wird geprüft (`bwrap --version`), sonst `SANDBOX_002` |
| `--unshare-all` | User-, IPC-, PID-, Net-, UTS-, Cgroup-Namespace. Net leer ⇒ nur `lo`. PID ⇒ Host-`/proc/<pid>/environ` unlesbar. IPC ⇒ kein SysV-shm mit Host |
| `--die-with-parent` | Stirbt der Daemon, stirbt die Sandbox. Keine Waisen mit Proxy-Zugang |
| `--new-session` | Eigene Session ⇒ kein `TIOCSTI` auf das Host-Terminal |
| `--hostname sandbox` | Kein Host-Hostname-Leak (`/proc/sys/kernel/hostname`) |
| `--ro-bind /usr /usr` | Toolchain read-only |
| `--symlink usr/lib /lib`, `--symlink usr/lib64 /lib64`, `--symlink usr/bin /bin`, `--symlink usr/sbin /sbin` | Merged-usr-Layout wie auf Debian |
| `--ro-bind-try /etc/alternatives /etc/alternatives` | Debian-Alternatives für `python3`, `editor` |
| `--ro-bind-try /etc/ld.so.cache /etc/ld.so.cache` | Loader-Cache |
| `--ro-bind /etc/ssl /etc/ssl` | System-Trust-Store als Basis |
| `--ro-bind <ca_bundle_src> /etc/ssl/certs/ca-certificates.crt` | Overlay: System-Bundle plus Humanitl-CA (nach dem `/etc/ssl`-Bind, sonst wird es überdeckt) |
| `--ro-bind <ca_cert_src> /etc/humanitl/ca.crt` | Für Env-Kit-Variablen |
| `--ro-bind-data <fd_passwd> /etc/passwd`, `--ro-bind-data <fd_group> /etc/group` | Minimal, nur der Sandbox-User; kein Host-`/etc/passwd` |
| `--ro-bind-data <fd_hosts> /etc/hosts` | Inhalt genau `127.0.0.1 localhost sandbox` |
| kein `/etc/resolv.conf` | Kein Resolver. Absichtlich. `getaddrinfo` scheitert sofort statt zu hängen |
| `--proc /proc` | Frisches procfs des PID-Namespaces |
| `--dev /dev` | Minimales devfs (`null`, `zero`, `random`, `urandom`, `tty`, `pts`, `shm` als tmpfs) |
| `--tmpfs /tmp` | Kein Host-`/tmp` (dort liegen X11-Sockets) |
| `--dir /home/agent`, `--setenv HOME /home/agent` | tmpfs-Home; Persistenz kommt mit HUM-037 (Volume für Agent-Config) |
| `--bind <work_src> /work` oder `--ro-bind <work_src> /work` | Projekt |
| pro Eintrag `mounts.tmpfs` mit Präfix `/work`: `--tmpfs <pfad>` nur wenn `<work_src>/<rel>` existiert | Maskierte Verzeichnisse (`.git/hooks`, `.vscode`, `.idea`). bwrap legt fehlende Mountpoints nicht in ro-Binds an, daher Existenzprüfung |
| pro Eintrag `mounts.masked_files`: `--ro-bind /dev/null <pfad>` nur wenn existiert | Maskierte Dateien (`.envrc`, `.git/config`) erscheinen leer |
| `--bind <proxy_socket_src> /run/humanitl/proxy.sock` | Die eine Tür (HUM-013). bwrap legt `/run/humanitl` als tmpfs-Verzeichnis an |
| `--ro-bind <shim_src> /usr/local/bin/humanitl-shim` | Launcher |
| `--chdir /work` | Agent startet im Projekt |
| `--clearenv` | Kein Host-Environment (Tokens, `DISPLAY`, `WAYLAND_DISPLAY`, `DBUS_SESSION_BUS_ADDRESS`) |
| `--setenv K V` pro Env-Kit-Eintrag (HUM-014) plus `PATH=/usr/local/bin:/usr/bin:/bin`, `TERM` aus Kontext, `LANG=C.UTF-8`, `USER=agent`, `HOME=/home/agent` | Vollständig kontrolliertes Environment |
| `--` | Ende der Optionen |
| `/usr/local/bin/humanitl-shim -- <agent_argv...>` | Der Shim setzt seccomp und startet den Agenten (HUM-012) |

Nicht in der Liste und verboten: `--share-net`, `--bind /run`, `--bind /tmp`, `--bind $XDG_RUNTIME_DIR`, `--bind /home`, `--dev-bind`. Der Argv-Builder lehnt Profile ab, die solche Pfade in `mounts.ro` oder `mounts.rw` enthalten (`SANDBOX_004`). Blockliste im Code: `/run`, `/tmp`, `/home`, `/root`, `/var/run`, `/proc`, `/sys`, `/dev`, Pfade unter `$XDG_RUNTIME_DIR`, jede Datei mit Modus `S_IFSOCK` außer der Proxy-Socket-Datei.

`--ro-bind-data` erwartet einen File-Descriptor: `LaunchPlan.fds` enthält `(host_fd, target_fd)`-Paare; beim Spawn werden sie per `pre_exec` mit `dup2` an die Ziel-Nummern gelegt und in der Argv als Zahl referenziert. Inhalt kommt aus `memfd_create` (keine Temp-Dateien).

Diagnostic-Codes:

| Code | Wann | why | fix |
|---|---|---|---|
| `SANDBOX_001` | `bwrap` nicht gefunden | „bubblewrap ist nicht installiert" | `CopyCommand("sudo apt install bubblewrap")` |
| `SANDBOX_002` | Version < 0.8 | Versionsstring | `CopyCommand` mit Paketbefehl |
| `SANDBOX_003` | `bwrap` bricht mit `EPERM`/„setting up uid map" ab | „Unprivilegierte User-Namespaces sind deaktiviert (Ubuntu: AppArmor `userns` Restriction, sonst `kernel.unprivileged_userns_clone`)" | `CopyCommand("sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0")` plus Docs-Link |
| `SANDBOX_004` | Profil enthält verbotenen Mount | Pfad | keine |
| `SANDBOX_005` | `work_src` existiert nicht oder ist keine Directory | Pfad | keine |
| `SANDBOX_006` | `work_src` nicht schreibbar bei `rw` | Pfad | `RemountReadOnly(path)` |
| `SANDBOX_007` | Proxy-Socket-Datei fehlt | Pfad | keine (interner Fehler) |
| `SANDBOX_008` | Shim-Binary fehlt oder nicht ausführbar | Pfad | keine |
| `SANDBOX_009` | bwrap beendet innerhalb 500 ms mit Fehler | stderr-Auszug (max 2 KB) | keine |

### Schritte
1. Crate anlegen, `SandboxProfile` aus HUM-010 hierher verschieben, `Cargo.toml` mit `tokio` (process, io-util), `nix` (unistd, fcntl, sys/memfd), `shell-escape`, `thiserror`.
2. `plan.rs`: `ArgvBuilder` mit Methoden je Tabellenzeile; `build(profile, ctx) -> Result<LaunchPlan, Diagnostic>`; Blockliste prüfen; Existenzprüfungen für tmpfs/masked; `argv_display` via `shell_escape::unix::escape`. Kompiliert, Snapshot-Test für `default.toml` grün.
3. `bwrap.rs`: `which bwrap`, Versionsprüfung (`bwrap --version` ⇒ `bubblewrap 0.11.0`), `BwrapBackend::plan` ruft Builder; `launch` spawnt mit `tokio::process::Command`, `pre_exec` dup2 der memfds, stdin/stdout/stderr als `Stdio` aus Kontext (Default `inherit` für CLI, `piped` für Daemon), nach Spawn 500 ms auf frühen Exit warten ⇒ `SANDBOX_009`.
4. `handle.rs`: `wait`, `kill` mit SIGTERM ⇒ 5 s ⇒ SIGKILL.
5. `isolation_check` liefert im MVP drei Ergebnisse aus den Prüfzeilen, die der Shim beim Start auf `HUMANITL_REPORT_FD` schreibt (`CHECK <name> ok|fail <evidence>`). Der Launcher liest sie aus dem `SandboxHandle` der laufenden Sandbox und bildet sie auf `NoNetworkInterface`, `SingleSocket` und `SeccompActive` ab; fehlt der Bericht, ist das `SANDBOX_013`. Entschieden am 2026-09-03 gegen die ursprünglich geplante zweite Sandbox mit `humanitl-shim --check`: Der Bericht stammt aus genau der Sandbox, in der der Agent läuft, kostet keinen zweiten Start und hat keinen eigenen Code-Pfad, der auseinanderlaufen könnte. Der Preis steht in den Fallstricken.
6. Profile `default.toml` (CONVENTIONS 3.4) und `test.toml` (identisch, aber `work` als `rw` auf ein tmp-Verzeichnis, für Escape-Tests) anlegen.

### Tests
- `argv_default_snapshot`: Profil `default.toml`, fester Kontext ⇒ Argv gleicht `tests/snapshots/argv_default.txt` (insta oder manueller Vergleich). Pfade im Kontext sind Platzhalter, damit der Snapshot stabil ist.
- `argv_ro_work`: `work_mode = Ro` ⇒ `--ro-bind <work> /work`, kein `--bind`.
- `argv_rejects_runtime_dir`: Profil mit `mounts.ro = ["/run/user/1000"]` ⇒ `Err(SANDBOX_004)`.
- `argv_rejects_socket_in_ro`: Profil mit `mounts.ro = ["/tmp/.X11-unix"]` ⇒ `Err(SANDBOX_004)`.
- `argv_masks_only_existing`: `work_src` ohne `.git` ⇒ kein `--tmpfs /work/.git/hooks`; mit `.git/hooks` ⇒ vorhanden.
- `launch_echo` (Integration, benötigt bwrap): `agent_argv = ["sh", "-c", "echo ok"]` mit Shim-Stub ⇒ stdout `ok`, Exit 0.
- `launch_no_interface` (Integration): `agent_argv = ["sh", "-c", "ip -o link | wc -l"]` ⇒ `1`, oder falls `ip` fehlt `ls /sys/class/net` ⇒ nur `lo`.
- `launch_no_resolv`: `test -e /etc/resolv.conf` ⇒ Exit 1.
- `launch_hostname`: `cat /proc/sys/kernel/hostname` ⇒ `sandbox`.
- `launch_env_clean`: `env` enthält keine Variable außer der gesetzten Liste.
- `launch_early_exit_diag`: `agent_argv = ["/nonexistent"]` ⇒ `SANDBOX_009` mit stderr im `why`.

### Akzeptanzkriterien
- [x] `cargo test -p humanitl-sandbox` grün, Integrationstests laufen auf dem CI-Runner mit bwrap. (192 Tests, mit bubblewrap 0.12.0 wirklich gestartet)
- [ ] `argv_display` einer Default-Session ist eine einzige Zeile, per Copy-Paste in einer Shell ausführbar (manuell geprüft, dokumentiert in `docs/SECURITY.md` als Beispiel). Eine Zeile ja; ausführbar nein: sie trägt `--json-status-fd 11` und `--ro-bind-data 12..16`, Deskriptoren, die eine Shell nicht hat (`launcher.rs:170-171` sagt es selbst); `docs/SECURITY.md:557` behauptet das Gegenteil und zeigt keine Beispielzeile (HUM-111).
- [x] Kein Pfad aus der Blockliste kann über ein Profil gemountet werden (Test).
- [ ] In der laufenden Sandbox: `ls /sys/class/net` zeigt nur `lo`; `/etc/resolv.conf` fehlt; `env | wc -l` ≤ 25. `/etc/resolv.conf` fehlt; `/sys` ist gar nicht gemountet, nur `lo` belegt der Shim über `/proc/net/dev`; `env | wc -l` ergibt 27 mit dem nackten Profil und rund 40 mit dem OpenCode-Adapter, nicht ≤ 25.
- [ ] Alle neun Diagnostic-Codes haben einen Unit-Test, der sie auslöst. Die neun der Spezifikation gibt es so nicht mehr (Register: `SANDBOX_001..007`, `010..016`); ohne auslösenden Test bleiben `SANDBOX_003`, `SANDBOX_004` und `SANDBOX_014` (Garantie 1 rot).

### Fallstricke
- `--unshare-all` schließt `--unshare-user` ein. Auf Ubuntu ≥ 24.04 blockiert AppArmor unprivilegierte User-Namespaces für Binaries ohne Profil; `bwrap` aus dem Ubuntu-Paket hat ein Profil, ein selbst gebautes nicht. Fehlerbild ist `bwrap: setting up uid map: Permission denied` ⇒ `SANDBOX_003`.
- Reihenfolge der Binds ist bedeutsam: spätere Binds überdecken frühere. Das CA-Bundle-Overlay muss nach `--ro-bind /etc/ssl` kommen.
- `--tmpfs` auf einem Pfad unterhalb eines `--ro-bind` funktioniert (bwrap mountet in Reihenfolge), aber nur wenn das Verzeichnis existiert.
- `--ro-bind /dev/null <datei>` funktioniert nur, wenn die Zieldatei existiert; sonst legt bwrap eine leere Datei an, was bei einem ro-Bind von `/work` scheitert. Daher Existenzprüfung.
- `memfd` für `--ro-bind-data` müssen ohne `O_CLOEXEC`-Vererbung an das Kind gehen; `tokio::process::Command::pre_exec` mit `dup2` auf feste Nummern (z. B. 10, 11, 12), und die Argv referenziert diese Nummern.
- `--die-with-parent` nutzt `PR_SET_PDEATHSIG`; das gilt für den Thread, der spawnt. Tokio-Multithread: Der Spawn-Thread kann enden, dann stirbt die Sandbox. Lösung: Spawn aus einem dedizierten `std::thread` mit langer Lebensdauer oder `tokio::task::spawn_blocking` mit gehaltener Handle-Referenz im Daemon; im Test mit `sleep 2` prüfen, dass die Sandbox lebt.
- Loopback: bwrap konfiguriert `lo` mit `127.0.0.1` beim `--unshare-net` selbst (Funktion `loopback_setup`). Ohne aktives `lo` könnte der Shim nicht binden. Test `launch_no_interface` bestätigt nebenbei, dass `lo` `UP` ist (`cat /sys/class/net/lo/operstate` ⇒ `unknown` oder `up`, nicht `down`).
- `--clearenv` entfernt auch `PATH`; ohne `--setenv PATH` findet `execvp` nichts.

### Referenzen
BACKLOG.md 3.1, 4.1, 4.5 (ESC-2), ADR-002; CONVENTIONS 3.4; bwrap-Manpage https://manpages.debian.org/trixie/bubblewrap/bwrap.1.en.html ; sandbox-runtime https://github.com/anthropic-experimental/sandbox-runtime ; Codex linux-sandbox https://github.com/openai/codex/blob/main/codex-rs/linux-sandbox/README.md

---

## HUM-012 · humanitl-shim
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-011 · Blockiert: HUM-013, HUM-021

### Kontext
Setzt die dritte Garantie aus BACKLOG.md 4.1 um („keine neuen Türen"). Der Agent braucht eine TCP-Verbindung zu `127.0.0.1:3128`, die auf den bind-gemounteten Unix-Socket weitergeleitet wird. Diese Brücke darf selbst Sockets öffnen, der Agent danach nur noch Loopback-TCP. Das Sicherheitsreview hat die Lücke „seccomp nach socat" benannt: Wird der Filter vor der Brücke gesetzt, kann die Brücke nicht starten; wird er zu spät gesetzt, läuft der Agent kurz ungefiltert. Der Shim löst das durch Prozesstrennung: Die Brücke lebt im Elternprozess ohne Filter, der Agent ist ein Kind mit Filter vor `exec`.

### Ziel
`humanitl-shim` ist ein statisches Binary (kein tokio, nur `std`, `libc`, `seccompiler`), das in der Sandbox als erster Prozess läuft: Es öffnet den Listener `127.0.0.1:3128`, forkt den Agenten mit seccomp-Filter, leitet jede TCP-Verbindung auf `/run/humanitl/proxy.sock` weiter, wartet auf das Kind und endet mit dessen Exit-Code. Auf `HUMANITL_REPORT_FD` schreibt er dabei die Prüfzeilen, aus denen der Launcher die drei Isolationsprüfungen macht.

### Nicht-Ziel
Kein socat-Abhängigkeit (Variante A unten ist Fallback, wird nicht gebaut). Kein PTY-Handling (HUM-042). Keine Signalweiterleitung vom Host über bwrap hinaus (HUM-067 behandelt `humanitl run`).

### Betroffene Pfade
- `daemon/bin/humanitl-shim/Cargo.toml` (neu): `[profile.release] opt-level="z", lto=true, panic="abort"`, Ziel `x86_64-unknown-linux-musl`
- `daemon/bin/humanitl-shim/src/main.rs` (neu)
- `daemon/bin/humanitl-shim/src/bridge.rs` (neu)
- `daemon/bin/humanitl-shim/src/seccomp.rs` (neu)
- `daemon/bin/humanitl-shim/src/check.rs` (neu)
- `tests/escape/esc-1.sh` (neu), `tests/escape/esc-2.sh` (neu)
- `docs/SECURITY.md`: Abschnitt „Shim und seccomp"

### Spezifikation

**Aufruf:** `humanitl-shim --proxy-port PORT -- AGENT ARGS...`. Die Brücken selbst kommen als JSON in `HUMANITL_BRIDGES`, die Filtertabelle in `HUMANITL_SECCOMP_FAMILIES`, `HUMANITL_SECCOMP_TYPES` und `HUMANITL_SECCOMP_DENY`; so steht keine Sicherheitsentscheidung in der Kommandozeile, die in `/proc` für jeden lesbar wäre. `humanitl-shim --rules` gibt die Tabelle ohne Agent aus.

**Prozessmodell (Variante B, verbindlich):**

```
main():
  parse args
  listener = TcpListener::bind("127.0.0.1:3128")        // vor fork, damit der Agent nie ECONNREFUSED sieht
  set CLOEXEC on listener                                 // Kind darf den Listener nicht erben
  pid = fork()
  if child:
      prctl(PR_SET_PDEATHSIG, SIGKILL)                    // stirbt der Shim, stirbt der Agent
      if getppid() != parent_pid_before_fork: _exit(1)    // Race: Parent schon tot
      close_range(3, ~0, 0)                               // keine geerbten fds außer 0,1,2
      prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)
      seccomp::apply()                                    // TSYNC, siehe unten
      execvp(agent[0], agent)                             // nie zurück; bei Fehler: stderr "humanitl-shim: exec failed: <errno>", _exit(127)
  parent:
      install SIGTERM/SIGINT handler: kill(child, sig)    // Weiterleitung
      spawn thread: accept loop
          for conn in listener.incoming():
              spawn thread: bridge(conn, proxy_sock)
      status = waitpid(child)
      exit(code)   // Exit-Code des Kindes; bei Signal n ⇒ 128+n
```

`bridge(conn, path)`: `UnixStream::connect(path)`; bei Fehler `conn` schließen (Client sieht Reset; HTTP-Client meldet „proxy connection failed"). Sonst zwei Threads mit `std::io::copy` in beide Richtungen; `shutdown(Write)` auf der jeweils anderen Seite, wenn eine Richtung EOF liefert. `TCP_NODELAY` auf `conn`. Kein Pufferlimit nötig, `copy` ist synchron.

**Bridge-Liste.** Der Shim liest `HUMANITL_BRIDGES` (JSON-Array, vom Launcher aus `[network].bridges` gesetzt): `[{"name":"proxy","dir":"in","listen":"127.0.0.1:3128","socket":"/run/humanitl/proxy.sock"}]`. Für jede `in`-Bridge ein Listener im Elternprozess; `out`-Bridges (Unix-Listener in der Sandbox, Weiterleitung an Sandbox-TCP) sind als `enum BridgeDir { In, Out }` modelliert und liefern im MVP `Diagnostic SANDBOX_007 "bridge direction out not supported yet"`. Die Familien-Liste für den seccomp-Filter kommt aus `HUMANITL_SECCOMP_FAMILIES` (Default `AF_INET,AF_INET6`); das Profil `browser` (Post-MVP) fügt `AF_UNIX` hinzu.

**Variante A (nicht bauen, dokumentieren):** `socat TCP-LISTEN:3128,bind=127.0.0.1,fork,reuseaddr UNIX-CONNECT:/run/humanitl/proxy.sock` als Kind starten, per Connect-Probe auf `127.0.0.1:3128` warten, dann Filter und exec. Nachteile: zusätzliches Binary im Image, Wartezeit-Race, kein Exit-Code-Durchgriff. Deshalb B.

**seccomp-Filter (`seccomp.rs`):** Blacklist mit Default `Allow`. seccompiler: `SeccompFilter::new(rules, mismatch_action = Allow, match_action = Errno(EPERM), arch)`. Ein Syscall, dessen Regel matcht, bekommt `EPERM`.

| Syscall | Regel (Bedingungen sind UND-verknüpft) | Begründung |
|---|---|---|
| `socket` | `arg0 != AF_INET` UND `arg0 != AF_INET6` | Nur Loopback-TCP zum Shim. `AF_UNIX` (kein anderer Socket ist erreichbar, trotzdem zu), `AF_NETLINK` (keine Interface-Konfiguration trotz CAP_NET_ADMIN im userns), `AF_PACKET`, `AF_VSOCK`, `AF_BLUETOOTH` etc. |
| `socketpair` | keine Bedingung | Wird nicht geblockt. Node/Bun nutzen `socketpair(AF_UNIX)` für Kindprozess-IPC; ein socketpair verbindet nur zwei fds desselben Prozessbaums, kein Egress. Diese Zeile steht hier, damit niemand sie „zur Sicherheit" blockt |
| `io_uring_setup` | keine Bedingung ⇒ EPERM | `IORING_OP_SOCKET` (seit 5.19) umgeht den `socket`-Filter |
| `ptrace` | keine Bedingung ⇒ EPERM | Kein Anhängen an die Brücke |
| `process_vm_readv`, `process_vm_writev` | ⇒ EPERM | Kein Speicherlesen der Brücke |
| `kexec_load`, `kexec_file_load`, `init_module`, `finit_module`, `delete_module`, `bpf`, `perf_event_open`, `userfaultfd` | ⇒ EPERM | Standard-Härtung, wie Docker-Default-Profil |

Anwenden: `seccompiler::apply_filter(&bpf)` (nutzt `seccomp(2)` mit `SECCOMP_FILTER_FLAG_TSYNC`). Vorher `PR_SET_NO_NEW_PRIVS`. Arch beim Build wählen (`TargetArch::x86_64`, später `aarch64` per `cfg`).

```rust
pub fn build_filter() -> Result<BpfProgram, seccompiler::Error> {
    use seccompiler::*;
    let deny = SeccompAction::Errno(libc::EPERM as u32);
    let mut rules: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
    rules.insert(libc::SYS_socket, vec![SeccompRule::new(vec![
        SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Ne, libc::AF_INET as u64)?,
        SeccompCondition::new(0, SeccompCmpArgLen::Dword, SeccompCmpOp::Ne, libc::AF_INET6 as u64)?,
    ])?]);
    for sc in [libc::SYS_io_uring_setup, libc::SYS_ptrace, libc::SYS_process_vm_readv, libc::SYS_process_vm_writev,
               libc::SYS_kexec_load, libc::SYS_init_module, libc::SYS_finit_module, libc::SYS_delete_module,
               libc::SYS_bpf, libc::SYS_perf_event_open, libc::SYS_userfaultfd] {
        rules.insert(sc, vec![]);   // leere Regelliste = matcht immer
    }
    SeccompFilter::new(rules, SeccompAction::Allow, deny, std::env::consts::ARCH.try_into()?)?.try_into()
}
```

Prüfen, ob eine leere Regelliste in seccompiler „matcht immer" bedeutet; falls nicht, eine Bedingung `arg0 >= 0` (immer wahr) einsetzen. Test `esc-1` entscheidet.

**Prüfzeilen auf `HUMANITL_REPORT_FD`** (eine Zeile je Prüfung, vor `exec` geschrieben):

```
CHECK no_interfaces ok /sys/class/net: lo
CHECK bridge_listening ok 127.0.0.1:3128 -> /run/humanitl/proxy.sock
CHECK single_socket ok sockets=/run/humanitl/proxy.sock;unexpected=none;entries=2000;limit=entries
CHECK seccomp_applied ok filter loaded, NoNewPrivs: 1
CHECK families ok AF_INET, AF_INET6
```

Implementierung: `no_interfaces` liest `/sys/class/net` und erwartet genau `lo`. `bridge_listening` meldet jede gebundene Brücke mit Ziel-Socket. `single_socket` läuft vor dem `exec` einen begrenzten Suchlauf über das Dateisystem der Sandbox (ohne `/proc`, `/sys` und `/dev` außer `/dev/shm`, ohne Symlinks zu folgen, Tiefe 3 und 2000 Einträge wie `SOCKET_WALK_MAX_DEPTH` und `SOCKET_WALK_MAX_ENTRIES`) und nennt jeden gefundenen Unix-Socket sowie die Schranke, falls eine griff; der Typ kommt aus `lstat`, weil `readdir` für den überbundenen Proxy-Socket „reguläre Datei" meldet. `seccomp_applied` und `families` melden den geladenen Filter und die erlaubten Familien. Der Launcher (HUM-011) macht daraus die drei Ergebnisse: `single_socket` und `bridge_listening` tragen zusammen Garantie 2 (keine zweite Tür, und die eine antwortet). `--check` als eigener Modus des Shims entfällt damit; `--rules` gibt die Filtertabelle für Menschen aus.

### Schritte
1. Crate mit musl-Target, `cargo build --release --target x86_64-unknown-linux-musl`, Binary < 2 MB. `main.rs` mit Arg-Parsing ohne clap (manuell, um Größe klein zu halten).
2. `bridge.rs` und Listener; Test außerhalb der Sandbox: `humanitl-shim --proxy-sock /tmp/x.sock -- sh -c 'curl -x 127.0.0.1:3128 http://example/'` gegen einen Test-UDS-Echo-Server.
3. `seccomp.rs`; Test: Kind mit Filter, `socket(AF_UNIX)` ⇒ `EPERM`, `socket(AF_INET)` ⇒ ok, `socketpair` ⇒ ok, `io_uring_setup` ⇒ `EPERM`.
4. Fork/exec-Ablauf mit `PDEATHSIG`, `close_range`, Signalweiterleitung, Exit-Code.
5. Prüfzeilen `CHECK <name> ok|fail <evidence>` auf `HUMANITL_REPORT_FD` vor dem `exec`.
6. `esc-1.sh`, `esc-2.sh` in `tests/escape/` gegen CONVENTIONS 3.11; Harness aus HUM-006 anbinden.
7. `BwrapBackend::isolation_check` (HUM-011 Schritt 5) auf die Prüfzeilen umstellen.

### Tests
- `bridge_roundtrip`: UDS-Echo, 1 MB Zufallsdaten hin und zurück, byteidentisch.
- `bridge_many_conns`: 200 parallele Verbindungen, alle erfolgreich.
- `bridge_proxy_down`: UDS existiert nicht ⇒ Client-Connect wird sofort geschlossen, Shim läuft weiter.
- `filter_denies_unix`, `filter_allows_inet`, `filter_allows_socketpair`, `filter_denies_io_uring`.
- `exit_code_passthrough`: Agent `sh -c 'exit 7'` ⇒ Shim-Exit 7; `sh -c 'kill -9 $$'` ⇒ 137.
- `no_inherited_fds`: Agent `ls /proc/self/fd` ⇒ genau `0 1 2` (plus das `ls`-eigene Verzeichnis-fd `3`).
- `esc-1.sh` (in Sandbox): Python oder ein kleines C/Rust-Testprogramm ruft `socket()` für AF_UNIX, AF_NETLINK, AF_PACKET, AF_INET6 (erlaubt), `socketpair`; `connect` von einem AF_INET-Socket an `10.0.0.1:80` ⇒ `ENETUNREACH`; `grep Seccomp /proc/self/status` ⇒ `2`. Grün, wenn alle Erwartungen stimmen.
- `esc-2.sh` (in Sandbox): `/proc/self/mountinfo` enthält keinen der Blocklisten-Pfade; `find / -xdev -type s 2>/dev/null` ⇒ genau `/run/humanitl/proxy.sock`; `cat /proc/1/environ` ist leer oder `bwrap`; `hostname` ⇒ `sandbox`; `ls /proc/self/fd` wie oben.

### Akzeptanzkriterien
- [x] `humanitl-shim` ist statisch (`ldd` ⇒ „not a dynamic executable"), < 2 MB. (1 340 328 Bytes)
- [x] Der Agent-Prozess hat `Seccomp: 2` und `NoNewPrivs: 1` in `/proc/<pid>/status`. Der Shim-Prozess trägt seit dem 2026-09-03 ebenfalls einen Filter (`Seccomp: 2`), dieselbe Sperrliste, nur zusätzlich `AF_UNIX` für die Brücke; damit trägt jeder Prozess unter bwraps Init einen Filter (ESC-1 `seccomp_every_process`).
- [x] `esc-1.sh` und `esc-2.sh` grün im CI-Job `escape-tests`. (heute `esc-1-sockets.sh` und `esc-2-mounts.sh`, 43 und 25 Fälle grün)
- [x] `curl -x http://127.0.0.1:3128 http://example.com/` aus der Sandbox erreicht den Daemon-Socket (sichtbar im Daemon-Log), bevor HUM-015 antwortet.
- [x] Exit-Code des Agenten kommt beim Aufrufer an.

### Fallstricke
- Filter im Elternprozess setzen wäre falsch: `TSYNC` synchronisiert Threads desselben Prozesses, nicht Kinder; aber der Elternprozess braucht `socket()`, also Filter nur im Kind, nach `fork`, vor `exec`. Der Filter vererbt sich auf alle Nachkommen des Agenten.
- `PR_SET_NO_NEW_PRIVS` ist Voraussetzung für seccomp ohne `CAP_SYS_ADMIN`; ohne ihn liefert `seccomp(2)` `EACCES`.
- `fork()` in einem Prozess mit Threads ist gefährlich; deshalb fork **vor** dem Start des Accept-Threads.
- `close_range` existiert seit Kernel 5.9 und glibc 2.34; Fallback: Schleife über `/proc/self/fd`.
- `listener.incoming()` blockiert; Thread-per-Connection ist hier absichtlich, Bun öffnet selten mehr als 50 Verbindungen. Keine `async`-Runtime im Shim.
- Nach `close_range` im Kind sind `stdin/stdout/stderr` noch offen (0,1,2), aber ein per `--ro-bind-data` von bwrap genutzter fd ist bereits geschlossen (bwrap schließt vor exec). Trotzdem `ls /proc/self/fd` im Test prüfen.
- `--new-session` in bwrap bedeutet: Ctrl+C im Host-Terminal erreicht die Sandbox nicht als Terminal-Signal. `humanitl sandbox run` (HUM-064) muss `SIGINT` explizit an bwrap senden; bwrap leitet an sein Kind (den Shim) weiter, der Shim an den Agenten.
- Das Docker-Default-Profil erlaubt `bpf` nicht; unsere Liste ebenfalls nicht. Falls ein Agent-Tool `bpf` braucht (unwahrscheinlich), Profil-Flag `seccomp.extra_allow` später.
- x32-ABI: seccompiler prüft die Architektur am Filteranfang; Syscalls mit x32-Bit (`__X32_SYSCALL_BIT`) werden von seccompiler als Fremdarchitektur behandelt und mit `mismatch_action` beantwortet. Bei `mismatch_action = Allow` wäre das ein Loch. Deshalb im Test explizit einen x32-`socket`-Aufruf (`syscall(0x40000000 | 41, ...)`) prüfen; wenn er durchgeht, `mismatch_action` auf `KillProcess` für fremde Arch setzen (seccompiler bietet dafür keinen separaten Schalter; dann eigenen BPF-Prolog voranstellen). Dokumentieren.

### Referenzen
BACKLOG.md 4.1, 4.5 (ESC-1, ESC-2), Security-Review Punkt 4 (seccomp-Lücke); CONVENTIONS 3.4, 3.11; seccompiler https://docs.rs/seccompiler ; seccomp(2) https://man7.org/linux/man-pages/man2/seccomp.2.html ; io_uring socket op https://man7.org/linux/man-pages/man3/io_uring_prep_socket.3.html

---

## HUM-013 · Proxy-Socket-Bind
Sprint: 1 · Größe: S · Abhängigkeiten: HUM-011, HUM-012 · Blockiert: HUM-015

### Kontext
Zweite Garantie („genau eine Tür"). Der Security-Review warnte: Liegen Proxy-Socket und gRPC-Socket im selben Verzeichnis und wird das Verzeichnis gemountet, hat der Agent die Steuer-API. Deshalb getrennte Verzeichnisse, Bind als Datei, nie als Verzeichnis, und ein Socket pro Session.

### Ziel
Der Daemon legt pro Session eine Socket-Datei `$XDG_RUNTIME_DIR/humanitl/proxy/<session-id>.sock` an (Verzeichnis 0700, Datei 0600), reicht deren Pfad als `proxy_socket_src` in den `SessionContext`, und die Sandbox sieht genau diese Datei unter `/run/humanitl/proxy.sock`. Der gRPC-Socket `$XDG_RUNTIME_DIR/humanitl/daemon.sock` und die Token-Datei sind in der Sandbox unsichtbar.

### Nicht-Ziel
Was hinter dem Socket passiert (HUM-015). Keine Mehrfach-Sessions-Verwaltung im UI.

### Betroffene Pfade
- `daemon/crates/proxy/src/listener.rs` (neu): `SessionSocket`
- `daemon/crates/sandbox/src/plan.rs`: Validierung, dass `proxy_socket_src` eine Socket-Datei ist
- `daemon/bin/humanitld/src/paths.rs` (neu): XDG-Pfade zentral (`Paths::runtime_dir()`, `Paths::proxy_dir()`, `Paths::daemon_socket()`, `Paths::token_file()`)

### Spezifikation

```rust
pub struct SessionSocket { pub path: PathBuf, listener: tokio::net::UnixListener }
impl SessionSocket {
    pub fn create(session: SessionId) -> Result<Self, Diagnostic>;   // mkdir -p proxy_dir 0700; unlink alte Datei; bind; chmod 0600
    pub fn listener(&self) -> &tokio::net::UnixListener;
}
impl Drop for SessionSocket { fn drop(&mut self) { let _ = std::fs::remove_file(&self.path); } }
```

Pfadregeln: `runtime_dir = $XDG_RUNTIME_DIR/humanitl` (Fallback `/run/user/<uid>/humanitl`; wenn beides fehlt ⇒ `DAEMON_003` „XDG_RUNTIME_DIR nicht gesetzt"). `proxy_dir = runtime_dir/proxy`. Beim Daemon-Start: `proxy_dir` leeren (verwaiste Sockets).

Bind in bwrap: `--bind <path> /run/humanitl/proxy.sock` (rw-Bind; ro-Bind funktioniert für Sockets technisch auch, weil `connect()` bei `S_IFSOCK` keine `EROFS`-Prüfung macht, aber rw ist eindeutiger und vermeidet Überraschungen bei Kernel-Änderungen). Der Argv-Builder prüft mit `metadata.file_type().is_socket()`, sonst `SANDBOX_007`.

### Schritte
1. `paths.rs` mit Tests für Fallbacks.
2. `SessionSocket` mit Rechten und Cleanup.
3. Argv-Builder-Validierung.
4. `esc-2.sh` um Prüfung ergänzen: `test -S /run/humanitl/proxy.sock`, `test ! -e /run/humanitl/daemon.sock`, `test ! -d /run/user`.

### Tests
- `socket_perms`: Verzeichnis 0700, Datei 0600, Eigentümer aktuelle UID.
- `socket_cleanup_on_drop`: Datei nach Drop weg.
- `socket_stale_replaced`: vorhandene Datei gleichen Namens wird ersetzt, Bind gelingt.
- `plan_rejects_non_socket`: reguläre Datei als `proxy_socket_src` ⇒ `SANDBOX_007`.
- `esc-2.sh` erweitert.

### Akzeptanzkriterien
- [ ] `find / -xdev -type s` in der Sandbox ⇒ genau `/run/humanitl/proxy.sock`. `-type s` findet nichts, weil bwrap den Socket über eine leere reguläre Datei bindet und `find` dem `d_type` aus `readdir` traut; das Produkt misst mit `-xtype s` (`tests/escape/lib.sh`, `report.rs:260-266`) und findet genau `/run/humanitl/proxy.sock`; `docs/SECURITY.md:125` druckt weiter die tote Form (HUM-111).
- [x] `ls -la $XDG_RUNTIME_DIR/humanitl/` zeigt `daemon.sock` (0600), `token` (0600), `proxy/` (0700).
- [ ] Zwei parallele Sessions haben zwei verschiedene Socket-Dateien und sehen jeweils nur ihre eigene. Es gibt genau einen Proxy-Socket `<runtime>/humanitl/proxy/proxy.sock` (`config/src/paths.rs:238-240`), `SessionSocket::create` (`listener.rs:91-93`) hat keinen Aufrufer, und `proxy_dir` wird beim Start nicht geleert; die zweite Sitzung ist auf Sprint 3 verschoben (CONVENTIONS 4.12), das Ziel `<session-id>.sock` dieses Issues wurde ohne Vermerk aufgegeben.

### Fallstricke
- Unix-Socket-Pfade sind auf 108 Bytes begrenzt (`sun_path`). `/run/user/1000/humanitl/proxy/<uuid>.sock` ist 58 Zeichen, sicher; bei langem `XDG_RUNTIME_DIR` ⇒ `DAEMON_004` mit Hinweis.
- Ein Bind auf eine Socket-Datei, die nach dem Bind vom Daemon neu erzeugt wird (unlink + bind), zeigt in der Sandbox auf den alten Inode. Deshalb: Socket zuerst erzeugen, dann Sandbox starten, Socket nie während der Session neu anlegen.
- Der bwrap-Mountpoint `/run/humanitl/` wird von bwrap als Verzeichnis in einem tmpfs angelegt; bwrap mountet `/run` nicht vom Host, weil es nicht in der Argv steht. Prüfen mit `mount | grep /run` in der Sandbox.

### Referenzen
BACKLOG.md 4.1 Garantie 2, Security-Review Punkt 3; CONVENTIONS 3.4 Pfade.

---

## HUM-014 · CA-Verwaltung und Env-Kit
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-011 · Blockiert: HUM-015, HUM-045

### Kontext
TLS-Interception braucht eine CA, der die Tools in der Sandbox vertrauen. Die CA darf nie im Host-Trust-Store landen (Security-Review 3). Jedes Tool hat seine eigene Umgebungsvariable; wer eine vergisst, sieht TLS-Fehler statt Sicherheitslücke (fail-closed), aber der Agent wird unbenutzbar. Deshalb hier die vollständige Liste als Tabelle.

### Ziel
`humanitl_proxy::ca::CaStore` erzeugt bei erstem Start eine CA (`ca.key` 0600, `ca.crt`), erzeugt daraus ein Bundle `ca-certificates.crt` (System-Bundle plus CA) und einen Java-Truststore, und liefert das Env-Kit als `Vec<(String, String)>`, das der Argv-Builder als `--setenv` setzt.

### Nicht-Ziel
Leaf-Zertifikat-Cache (HUM-015). Keyring-Unlock oder ephemere CA pro Session (Post-MVP, in `docs/SECURITY.md` als Option notieren).

### Betroffene Pfade
- `daemon/crates/proxy/src/ca.rs` (neu)
- `daemon/crates/proxy/src/envkit.rs` (neu)
- `daemon/crates/sandbox/src/plan.rs`: nimmt Env-Kit aus `SessionContext.extra_env`
- `profiles/sandbox/default.toml`: `[env]`-Sektion referenziert Kit-Schlüssel
- `docs/SECURITY.md`: Abschnitt „CA"

### Spezifikation

**CA-Erzeugung** mit `rcgen`: `CertificateParams` mit `is_ca = IsCa::Ca(BasicConstraints::Unconstrained)`, `key_usages = [KeyCertSign, CrlSign]`, CN `Humanitl Local CA <kurz-id>`, Gültigkeit 10 Jahre, Schlüssel ECDSA P-256. `<kurz-id>` = 8 Hex aus `machine-id`-Hash, damit zwei Installationen unterscheidbar sind. Dateien: `$XDG_DATA_HOME/humanitl/ca/ca.key` (PEM, 0600), `ca.crt` (PEM, 0644), `ca-bundle.crt` (System-Bundle `/etc/ssl/certs/ca-certificates.crt` + `ca.crt`, wird bei jedem Daemon-Start neu erzeugt, damit System-Updates ankommen). Der PKCS#12-Truststore `cacerts.p12` für die JVM ist am 2026-09-03 aus M1 herausgenommen: Der Agent im MVP ist OpenCode auf Node und Bun, keine JVM, und ein Truststore, den niemand liest, wäre eine Datei mehr, die in die Sandbox eingehängt wird und deren Passwort in einer Umgebungsvariablen steht. Er kommt zurück, sobald ein Java-Adapter dazukommt; bis dahin entfallen `cacerts.p12`, `JAVA_TOOL_OPTIONS` und der zugehörige Bind.

```rust
pub struct CaStore { pub key_pem: Zeroizing<String>, pub cert_pem: String, pub cert_path: PathBuf, pub bundle_path: PathBuf, pub p12_path: Option<PathBuf> }
impl CaStore {
    pub fn load_or_create(data_dir: &Path) -> Result<Self, Diagnostic>;   // TLS_001 bei Schreibfehler, TLS_002 bei korrupter Datei
    pub fn rcgen_authority(&self) -> Result<hudsucker::certificate_authority::RcgenAuthority, Diagnostic>;
    pub fn fingerprint_sha256(&self) -> String;   // für UI
}
```

**Env-Kit (verbindlich, alle setzen):**

| Variable | Wirkt auf | Wert in der Sandbox |
|---|---|---|
| `HTTP_PROXY`, `http_proxy` | curl, wget, Python, Go, Rust reqwest, Node undici, Bun, git | `http://127.0.0.1:3128` |
| `HTTPS_PROXY`, `https_proxy` | dito | `http://127.0.0.1:3128` |
| `ALL_PROXY` | curl, einige Go-Tools | `http://127.0.0.1:3128` |
| `NO_PROXY`, `no_proxy` | alle | leer (`""`). Nicht weglassen: manche Images setzen Defaults |
| `SSL_CERT_FILE` | OpenSSL, Go, Rust (rustls-native-certs), Ruby, Python httpx | `/etc/humanitl/ca.crt` |
| `SSL_CERT_DIR` | OpenSSL | `/etc/ssl/certs` |
| `CURL_CA_BUNDLE` | curl | `/etc/humanitl/ca.crt` |
| `REQUESTS_CA_BUNDLE` | Python requests | `/etc/humanitl/ca.crt` |
| `PIP_CERT` | pip | `/etc/humanitl/ca.crt` |
| `NODE_EXTRA_CA_CERTS` | Node, Bun ≥ 1.1.22 | `/etc/humanitl/ca.crt` |
| `NPM_CONFIG_CAFILE` | npm | `/etc/humanitl/ca.crt` |
| `DENO_CERT` | Deno | `/etc/humanitl/ca.crt` |
| `GIT_SSL_CAINFO` | git | `/etc/humanitl/ca.crt` |
| `CARGO_HTTP_CAINFO` | cargo | `/etc/humanitl/ca.crt` |
| `AWS_CA_BUNDLE` | AWS CLI/SDKs | `/etc/humanitl/ca.crt` |
| `GOFLAGS` | Go | nicht setzen; Go nutzt `SSL_CERT_FILE` |
| `HUMANITL_SESSION` | Agent-Tools, Diagnose | `<session-id>` |
| `HUMANITL` | Erkennung „läuft in Humanitl" | `1` |

Zusätzliche Binds (Argv-Builder): `--ro-bind <ca-bundle.crt> /etc/ssl/certs/ca-certificates.crt`, damit die Humanitl-CA in der Sandbox als System-Wurzel gilt.

Diagnostic-Codes (Stand nach dem Register in `codes.rs`, das hier vorgeht): `TLS_001` ist schon vergeben (Client hat die Humanitl-CA abgelehnt). Für dieses Issue gelten `TLS_004` Schreibfehler CA-Verzeichnis (fix: `CopyCommand("mkdir -p … && chmod 700 …")`) und `TLS_005` CA-Dateien unbrauchbar (fix: `CopyCommand("rm -r $XDG_DATA_HOME/humanitl/ca")` mit Warnung). Ein einzelnes Leaf, das nicht ausgestellt oder geprüft werden kann, ist ebenfalls `TLS_005`, aber ohne diesen Fix, weil die CA selbst dabei heil ist.

### Schritte
1. `ca.rs`: Erzeugen, Laden, Bundle bauen, Fingerprint.
2. `envkit.rs`: `pub fn env_kit(session: SessionId, has_p12: bool) -> Vec<(String, String)>`.
3. Integration in `SessionContext` und Argv-Builder; Binds ergänzen.
4. Dokumentation in `SECURITY.md`: Warum die CA nie auf den Host-Store darf, Fingerprint-Anzeige.

### Tests
- `ca_create_then_load`: zweimal `load_or_create` ⇒ gleicher Fingerprint; Dateirechte 0600/0644.
- `bundle_contains_both`: Bundle enthält System-Roots (Anzahl > 100) und die CA (letzter Block).
- `envkit_complete`: Alle Tabellenzeilen vorhanden, `NO_PROXY` ist leer, keine Variable enthält Host-Pfade.
- `sandbox_tools_trust_ca` (Integration, Sandbox mit HUM-015-Stub, der ein selbstsigniertes Leaf für `example.test` ausstellt): `curl https://example.test/` ⇒ kein TLS-Fehler; `python3 -c "import urllib.request; urllib.request.urlopen('https://example.test/')"` ⇒ ok; `node -e "fetch('https://example.test/')"` falls Node vorhanden; `git ls-remote https://example.test/repo` ⇒ Fehler ist HTTP, nicht TLS.

### Akzeptanzkriterien
- [x] `ls -la $XDG_DATA_HOME/humanitl/ca/` zeigt `ca.key` 0600, `ca.crt` 0644, `ca-bundle.crt`.
- [x] `trust list` oder `ls /etc/ssl/certs` auf dem Host enthält die Humanitl-CA **nicht**.
- [x] In der Sandbox: `env | grep -c -E 'PROXY|CA|CERT'` ≥ 16. (gemessen: 16)
- [x] `openssl verify -CAfile /etc/humanitl/ca.crt <leaf>` in der Sandbox ⇒ `OK`. (gemessen gegen einen laufenden `humanitld`; ein automatisierter Test dafür aus der Sandbox heraus fehlt, HUM-087 fährt M2 über `https://`)

### Fallstricke
- `NO_PROXY` mit `localhost` würde den Proxy für Loopback umgehen; da der Proxy selbst auf Loopback liegt, ist das egal, aber `NO_PROXY=*` in einem Agent-Image wäre fatal. Deshalb explizit leer setzen und `--clearenv`.
- Go akzeptiert `SSL_CERT_FILE` nur, wenn das System-Bundle nicht zuerst gefunden wird; da wir das Bundle überlagern, ist beides abgedeckt.
- Bun vor 1.1.22 ignoriert `NODE_EXTRA_CA_CERTS`. OpenCode bringt ein eigenes Bun mit; Version im Agent-Profil prüfen (HUM-037).
- `rcgen` erzeugt standardmäßig Zertifikate mit `NotBefore = jetzt`; bei Uhrzeitversatz in der Sandbox (nicht möglich, gleiche Uhr) unkritisch. Trotzdem `NotBefore = jetzt - 1 Tag`.
- Den Private Key nie in `LaunchPlan` oder Logs schreiben; `Zeroizing<String>` und `tracing`-Felder ausschließen.

### Referenzen
BACKLOG.md 4.4; Sandbox-Recherche Abschnitt 3 (TLS); Node https://nodejs.org/learn/http/enterprise-network-configuration ; Bun 1.1.22 https://bun.com/blog/bun-v1.1.22 ; rustls-native-certs https://docs.rs/rustls-native-certs ; rcgen https://docs.rs/rcgen

---

## HUM-015 · MITM-Proxy-Kern
Sprint: 1 · Größe: L · Abhängigkeiten: HUM-004, HUM-013, HUM-014 · Blockiert: HUM-016, HUM-017

### Kontext
ADR-001 (hudsucker) und ADR-005 (Body vollständig puffern). Der Proxy ist der Ort, an dem aus einer TCP-Verbindung ein `Flow` wird. Hier wird terminiert, gepuffert, die Authority extrahiert, an die Hold-Queue übergeben und nach Entscheidung weitergeleitet oder mit 403 beantwortet.

### Ziel
`humanitl_proxy::ProxyCore` nimmt Verbindungen vom Session-Socket entgegen, führt HTTP/1.1-Requests und CONNECT-Tunnel mit TLS-Terminierung durch, puffert den Request-Body bis zum Cap, erzeugt einen `Flow` im Zustand `Received`, übergibt ihn an einen `FlowPipeline`-Trait (Findings, Regeln, Hold; in diesem Issue ein Stub, der immer `Ask` liefert und die Hold-Queue aus HUM-016 nutzt) und leitet nach `Allow` weiter (Upstream über HTTP/1.1, DNS erst jetzt) oder antwortet mit 403.

### Nicht-Ziel
Regeln (HUM-022), Findings (HUM-025), Recorder (HUM-026), Authority/SNI-Konsistenzprüfung (HUM-023, hier nur Extraktion und Speicherung des CONNECT-Ziels), HTTP/2 zum Upstream (Flag `experimental.h2_upstream`, Default aus), WebSocket-Hold (Passthrough nach Freigabe des Upgrade-Requests, Frames werden in HUM-026 aufgezeichnet).

### Betroffene Pfade
- `daemon/crates/proxy/Cargo.toml`: `hudsucker = { version = "0.25", default-features = false, features = ["rcgen-ca", "rustls-client", "http2"] }`, `hyper`, `hyper-util`, `http-body-util`, `rustls`, `webpki-roots`, `tokio`, `bytes`, `dashmap`
- `daemon/crates/proxy/src/lib.rs`, `core.rs` (neu), `handler.rs` (neu), `body.rs` (neu), `upstream.rs` (neu), `bridge.rs` (neu), `pipeline.rs` (neu), `diag.rs` (Codes `PROXY_001..`)

### Spezifikation

**Egress-Port (`egress/mod.rs`, `egress/direct.rs`).** Alle Upstream-Verbindungen laufen über `trait Egress { async fn connect(&self, authority: &Authority, resolved: Option<IpAddr>) -> Result<Box<dyn AsyncStream>, Diagnostic>; }`. MVP-Implementierung `Direct`: `TcpStream::connect((resolved_ip, port))` mit Timeout `upstream.connect_timeout_secs` (Default 10). hudsuckers eigener Client wird deshalb nicht benutzt; stattdessen `hyper_util::client::legacy::Client` mit einem eigenen `Connector`, der `Egress` aufruft und für `https` rustls darüberlegt. Spätere Adapter `HttpProxy`, `Socks5h` (ADR-017) ersetzen nur diese Datei.

**Listener-Brücke (`bridge.rs`).** hudsucker 0.25 bietet `ProxyBuilder::with_listener(tokio::net::TcpListener)`, aber keinen Unix-Listener. Deshalb: pro Session ein `TcpListener` auf `127.0.0.1:0` (ephemerer Port, nur Loopback), und eine Aufgabe, die `UnixListener`-Verbindungen (HUM-013) annimmt und per `tokio::io::copy_bidirectional` an den TCP-Port weiterreicht. Session-Zuordnung: ein hudsucker-`Proxy` pro Session, Handler mit `session_id` im Zustand. Vor der Implementierung im hudsucker-Quelltext prüfen, ob `with_listener` inzwischen ein generisches `Listener`-Trait akzeptiert; falls ja, `UnixListener` direkt nutzen und die Brücke weglassen (im Code als `// TODO(HUM-015): remove loopback bridge once hudsucker accepts UnixListener` markieren). Der Loopback-Port ist für andere Host-Prozesse erreichbar; das ist im MVP akzeptiert (sie landen in der Queue, kein Egress ohne Freigabe) und in `SECURITY.md` dokumentiert.

**Handler (`handler.rs`).**

```rust
#[derive(Clone)]
pub struct FlowHandler {
    session: SessionId,
    pipeline: Arc<dyn FlowPipeline>,
    limits: Limits,                          // body_cap_bytes, preview_cap_bytes
    connect_authority: Option<Authority>,    // pro Verbindung, von CONNECT gesetzt
}

#[async_trait]
pub trait FlowPipeline: Send + Sync {
    /// Nimmt den vollständig gepufferten Request, liefert die Entscheidung. Blockiert bis zur Entscheidung.
    async fn decide(&self, flow: FlowId, req: &HttpRequest, meta: &ConnMeta) -> Decision;
    /// Wird nach der Antwort aufgerufen (Header sofort, Body gestreamt).
    fn on_response_headers(&self, flow: FlowId, status: u16, headers: &HeaderMap);
    fn on_response_chunk(&self, flow: FlowId, chunk: &Bytes);
    fn on_response_end(&self, flow: FlowId);
}
pub struct ConnMeta { pub session: SessionId, pub connect_authority: Option<Authority>, pub tls: bool }

impl HttpHandler for FlowHandler {
    async fn handle_request(&mut self, ctx: &HttpContext, req: Request<Body>) -> RequestOrResponse { ... }
    async fn handle_response(&mut self, ctx: &HttpContext, res: Response<Body>) -> Response<Body> { ... }
    async fn should_intercept(&mut self, _ctx: &HttpContext, req: &Request<Body>) -> bool { true }  // jede CONNECT-Verbindung wird terminiert
}
```

Ablauf `handle_request`:

1. Ist `req.method() == CONNECT`: `connect_authority = Some(parse(req.uri().authority()))` speichern, `RequestOrResponse::Request(req)` zurückgeben (hudsucker baut den Tunnel und ruft für jede entschlüsselte Anfrage erneut `handle_request`).
2. Authority bestimmen: bei absoluter URI aus `req.uri()`, sonst aus `Host`-Header; Port-Default 80/443 nach Schema; Schema `https` wenn `ctx`/`connect_authority` TLS anzeigt. Normalisieren zu `HostName` (CONVENTIONS 3.3). Fehlt beides ⇒ `400` mit Body `missing host`.
3. `FlowId::new()` (UUIDv7), `Received`-Event über Pipeline.
4. Body puffern: `http_body_util::Limited::new(body, body_cap)` und `.collect().await`; bei `LengthLimitError` ⇒ `Decision::Block { reason: BodyCap }` ohne Pipeline-Aufruf, sofort 403. hyper sendet `100 Continue` automatisch beim ersten Body-Poll, wenn `Expect: 100-continue` gesetzt ist; das ist gewollt, denn der Body muss vor der Entscheidung vorliegen. Weitergeleitet wird vor der Entscheidung nichts.
5. `HttpRequest` bauen (`BodyRef` mit sha256, size, `inline` wenn ≤ `recorder.inline_max_bytes`, sonst Bytes im Handler halten bis Forward).
6. `decision = pipeline.decide(flow, &req, &meta).await` (blockiert bis UI/Regel/Timeout).
7. `Allow` ⇒ Request mit `Full<Bytes>`-Body rekonstruieren, `Forwarded`-Event, `RequestOrResponse::Request`. `AllowEdited { request }` ⇒ editierten Request nehmen; die Authority darf sich durch die Bearbeitung nicht ändern (sonst `403 AuthorityMismatch`). `Block { reason }` / `TimedOut` ⇒ `RequestOrResponse::Response(blocked_response(flow, reason, host))`.

`blocked_response`: Status 403, `Content-Type: text/plain; charset=utf-8`, Header `X-Humanitl-Flow: <id>`, Body exakt nach CONVENTIONS 3.5.

`handle_response`: `on_response_headers`; Body durch einen `tee`-Wrapper leiten (`body.rs`: `TeeBody`, ruft `on_response_chunk` pro Frame, `on_response_end` bei EOF/Trailer), unverändert an den Client zurück. Kein Puffern. Trailer werden durchgereicht (`Frame::trailers`).

**Upstream (`upstream.rs`).** Eigener `hyper_util::client::legacy::Client` mit `HttpsConnector` aus `hyper-rustls`, rustls-`ClientConfig` mit `webpki-roots`, `alpn_protocols = [b"http/1.1"]` (h2 nur mit Flag). DNS: Standard-`GaiResolver` des `HttpConnector`; da der Client erst nach `decide` aufgerufen wird, findet die Auflösung nach der Entscheidung statt (ADR-006). Für IP-Pinning pro Verbindung genügt das Verhalten des Connectors (eine Auflösung pro Connect). Kein Connection-Pooling über Authorities hinweg: `pool_max_idle_per_host(2)`, Pool ist per Authority getrennt. An hudsucker via `.with_client(client)`.

**CA.** `RcgenAuthority::new(key_pair, ca_cert, cache_size = 1000, aws_lc_rs::default_provider())` aus `CaStore` (HUM-014). Leaf-Zertifikate werden von hudsucker aus dem CONNECT-Ziel erzeugt und gecacht.

**Aufbau (`core.rs`).**

```rust
pub struct ProxyCore { sessions: DashMap<SessionId, SessionProxy> }
pub struct SessionProxy { pub socket: SessionSocket, loopback_port: u16, task: JoinHandle<()>, bridge: JoinHandle<()> }
impl ProxyCore {
    pub async fn start_session(&self, session: SessionId, ca: &CaStore, pipeline: Arc<dyn FlowPipeline>, limits: Limits) -> Result<PathBuf /* socket path */, Diagnostic>;
    pub async fn stop_session(&self, session: SessionId);
}
```

Diagnostic-Codes: `PROXY_001` Loopback-Bind fehlgeschlagen, `PROXY_002` hudsucker-Start fehlgeschlagen, `PROXY_003` Upstream-Connect-Fehler (wird als 502 mit Body `upstream: <fehler>` beantwortet und als `Diagnostic` im Event mitgegeben), `PROXY_004` Body-Cap überschritten (Info, Flow ist geblockt).

### Schritte
1. Crate-Skelett, `CaStore` einbinden, `RcgenAuthority` bauen, leerer Handler, `ProxyCore::start_session` mit Loopback-Listener; `curl -x 127.0.0.1:<port> http://example.com` durchleiten (noch ohne Hold).
2. `bridge.rs`: UDS ⇒ Loopback. Test mit `socat`/`curl --unix-socket`.
3. Authority-Extraktion, `HostName`-Normalisierung (Funktion in `humanitl-core` wiederverwenden), CONNECT-Speicherung.
4. `body.rs`: `Limited`-Puffern, `BodyRef`, `TeeBody`.
5. `FlowPipeline`-Trait und `AskStubPipeline` (nutzt `HoldQueue` aus HUM-016; bis HUM-016 fertig ist: ein Stub, der sofort `Allow` liefert, um Schritt 1–4 zu testen).
6. `upstream.rs` mit ALPN h1 und Pool-Einstellungen; `.with_client`.
7. 403-Antwort, `AllowEdited`-Pfad mit Authority-Gleichheitsprüfung.
8. `handle_response` mit Tee.

### Tests
(Integrationstests in `daemon/crates/proxy/tests/`, mit axum-Fake-Upstream aus HUM-017; wo HUM-017 noch fehlt, ein minimaler `/echo`-Server im Test.)
- `plain_http_forward`: GET über Proxy an `http://127.0.0.1:<fake>/echo` ⇒ Antwort identisch, Flow-Events `Received`, `Forwarded`, `ResponseHeaders`, `Recorded` in Reihenfolge.
- `connect_tls_mitm`: `curl --cacert ca.crt --proxy … https://localhost:<fake-tls>/echo` ⇒ 200; Zertifikat des Proxys hat CN/SAN `localhost` und ist von der Humanitl-CA signiert.
- `body_buffered_before_decide`: POST 1 MB; Pipeline-Mock prüft, dass `req.body.size == 1 MB` beim `decide`-Aufruf und dass der Upstream vor `decide` keine Bytes gesehen hat.
- `body_cap_blocks`: POST `body_cap + 1` ⇒ 403 mit `reason: body_cap`, Upstream nicht kontaktiert.
- `expect_100_continue`: `curl -H 'Expect: 100-continue' --data-binary @1mb` ⇒ Upstream erst nach `Allow` kontaktiert.
- `block_returns_403`: Pipeline liefert `Block{User}` ⇒ 403, Body enthält `reason: user`, Header `X-Humanitl-Flow`.
- `allow_edited_changes_body`: Pipeline liefert `AllowEdited` mit anderem Body ⇒ Upstream sieht den neuen Body; Änderung der Authority ⇒ 403 `authority_mismatch`.
- `dns_after_decide`: Upstream-Host `held.test` mit Test-Resolver (Feature-Hook im Connector); Resolver-Aufrufzähler ist 0 vor `decide`, 1 danach.
- `alpn_h1_only`: TLS-Fake-Upstream mit h2-Angebot ⇒ ausgehandelt `http/1.1`.
- `response_streams`: `/sse` mit 5 Events à 1 s ⇒ Client empfängt das erste Event < 1,5 s nach Freigabe (kein Puffern), `on_response_chunk` 5× aufgerufen.
- `trailers_passthrough`: gRPC-Echo ⇒ `grpc-status`-Trailer beim Client vorhanden.

### Akzeptanzkriterien
- [ ] Aus der Sandbox: `curl -sS https://example.com` (mit Env-Kit) landet als `Received` im Daemon-Log, ohne dass der Host-Resolver angefragt wurde (prüfen mit `resolvectl statistics` oder `tcpdump -i any port 53` während des Holds).
- [ ] Alle Tests oben grün; kein `unwrap` außerhalb von Tests.
- [ ] 403-Body entspricht byteweise CONVENTIONS 3.5.
- [ ] `experimental.h2_upstream = true` schaltet ALPN auf `[h2, http/1.1]` (Test), Default bleibt h1.

### Fallstricke
- hudsucker braucht `tokio` mit `rt-multi-thread`; unter `#[tokio::test]` `flavor = "multi_thread"` setzen.
- `Limited` liefert bei Überschreitung einen Fehler erst beim Lesen; Content-Length vorher prüfen und bei `> cap` sofort blocken, ohne den Body zu lesen (spart Bandbreite, und der Client bekommt schnell 403).
- Bei `Block` muss der restliche Request-Body des Clients verworfen werden, sonst blockiert Keep-Alive; hyper macht das beim Drop des Bodies, wenn die Antwort `Connection: close` trägt. 403-Antwort mit `Connection: close` senden.
- Der `Host`-Header kann bei HTTP/1.1 über Proxy von der absoluten URI abweichen; die URI gewinnt (RFC 7230 §5.4), aber beides wird für HUM-023 gespeichert.
- `RcgenAuthority` erzeugt Leafs mit dem CONNECT-Host als SAN; sendet der Client ein anderes SNI, passt das Zertifikat nicht und der Client bricht ab. Das ist erwünscht (kein Fronting), muss aber als `TLS_003`-Diagnostic sichtbar werden (HUM-045).
- Kein `Body::collect()` auf Responses, nie. Streaming ist Pflicht (LLM-Antworten).
- IPv6-Literal-Authority `[::1]:8080` korrekt parsen (`http::uri::Authority`).

### Referenzen
ADR-001, ADR-005, ADR-006; Proxy-Recherche Abschnitt 2; hudsucker https://github.com/omjadas/hudsucker und https://docs.rs/hudsucker/0.25 ; hyper 1 Trailer https://github.com/hyperium/hyper/discussions/3620 ; http-body-util Limited https://docs.rs/http-body-util

---

## HUM-016 · Hold-Queue
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-004, HUM-015 · Blockiert: HUM-017, HUM-018

### Kontext
ADR-004. Ein gehaltener Request ist ein `Future`, das auf eine Entscheidung wartet. Die Queue darf die Proxy-Task nie blockieren, muss Deadlines durchsetzen (Timeout ⇒ Block, nie Allow) und muss den Zustandsautomaten aus `humanitl-core` durchlaufen, damit jedes Event konsistent ist.

### Ziel
`humanitl_proxy::hold::HoldQueue` und `FlowRegistry` verwalten alle Flows einer Session: Zustand pro Flow, Deadline-Timer, Entscheidung von außen (gRPC oder Terminal), und ein `broadcast`-Kanal für `FlowEvent`s. Die `AskPipeline` implementiert `FlowPipeline` mit dieser Queue: jeder Request wird gehalten, bis `decide()` gerufen wird oder die Deadline abläuft.

### Nicht-Ziel
Regel-Auswertung (HUM-022), Findings (HUM-025), Persistenz (HUM-026). Hier ist alles in-memory; nach Daemon-Neustart ist die Queue leer.

### Betroffene Pfade
- `daemon/crates/proxy/src/hold.rs` (neu)
- `daemon/crates/proxy/src/registry.rs` (neu)
- `daemon/crates/proxy/src/pipeline.rs`: `AskPipeline`
- `daemon/crates/core-types/src/flow.rs`: ggf. Hilfsfunktionen für Event-Ableitung

### Spezifikation

```rust
pub struct HoldQueue { pending: DashMap<FlowId, oneshot::Sender<Decision>> }
impl HoldQueue {
    pub fn hold(&self, id: FlowId, deadline: Instant) -> impl Future<Output = Decision> + Send;
    // intern: oneshot anlegen, in `pending` eintragen, `tokio::select!` zwischen rx und `sleep_until(deadline)`;
    // bei Timeout: Eintrag entfernen, `Decision::TimedOut` liefern.
    pub fn decide(&self, id: FlowId, d: Decision) -> Result<(), NotHeld>;   // entfernt Eintrag, sendet; NotHeld wenn unbekannt oder schon entschieden
    pub fn pending_ids(&self) -> Vec<FlowId>;
    pub fn extend(&self, id: FlowId, by: Duration) -> Result<Instant, NotHeld>;   // „Timer pausieren" im UI = extend um 24 h, audit-geloggt (HUM-050)
}

pub struct FlowRegistry {
    flows: DashMap<FlowId, FlowRecord>,
    events: broadcast::Sender<FlowEvent>,     // Kapazität ipc.event_buffer (1024)
}
pub struct FlowRecord { pub id: FlowId, pub session: SessionId, pub state: FlowState, pub request: HttpRequest, pub meta: ConnMeta, pub created: DateTime<Utc>, pub deadline: Option<Instant>, pub decision: Option<Decision>, pub response_status: Option<u16> }
impl FlowRegistry {
    pub fn insert(&self, rec: FlowRecord);
    pub fn transition(&self, id: FlowId, ev: FlowEvent) -> Result<(), InvalidTransition>;   // state = state.on(&ev)?; events.send(ev)
    pub fn get(&self, id: FlowId) -> Option<FlowRecord>;
    pub fn subscribe(&self) -> broadcast::Receiver<FlowEvent>;
    pub fn list(&self, filter: &FlowFilter) -> Vec<FlowSummary>;   // in-memory; ab HUM-026 aus SQLite
}
```

`AskPipeline::decide`:
1. `registry.insert(Received)`; `transition(Analyzed { findings: vec![] })` (Findings ab HUM-025).
2. `deadline = now + hold.timeout_secs`; `transition(Held { deadline })`.
3. `decision = queue.hold(id, deadline).await`.
4. `transition(Decided(decision.clone()))`; bei `Block`/`TimedOut` zusätzlich `transition(Recorded)` erst nach `on_response_end` des 403 (HUM-015 ruft die Response-Hooks auch für eigene 403-Antworten auf, damit die Kette vollständig ist).
5. Rückgabe.

`extend` ist nur erlaubt, solange `state == Held`. `decide` mit `Decision::TimedOut` von außen ist verboten (`NotHeld`), Timeout entsteht nur intern.

Broadcast-Lag: Empfänger, die `RecvError::Lagged(n)` sehen, erhalten stattdessen `FlowEvent::Lagged { n }` (Umwandlung im gRPC-Stream, HUM-018) und laden per `ListFlows` nach.

### Schritte
1. `HoldQueue` mit `select!`-Timeout.
2. `FlowRegistry` mit Übergängen, Fehler bei ungültigem Übergang wird geloggt (`tracing::error`) und als `PROXY_005` Diagnostic im Event-Stream weitergegeben, der Flow wird geblockt (fail-closed).
3. `AskPipeline`.
4. Anbindung in `ProxyCore::start_session`.

### Tests
- `hold_resolves_on_decide`: `hold` läuft, `decide(Allow)` ⇒ Future liefert `Allow` in < 10 ms.
- `hold_times_out`: `timeout = 200 ms`, keine Entscheidung ⇒ `TimedOut` nach 200 ± 50 ms.
- `decide_unknown_is_error`: `decide` auf fremde ID ⇒ `NotHeld`.
- `decide_twice_is_error`: zweites `decide` ⇒ `NotHeld`.
- `extend_moves_deadline`: `extend(1 s)` bei `timeout = 200 ms` ⇒ Entscheidung nach 500 ms kommt noch durch.
- `timeout_never_allows`: 1000 Flows mit Timeout ⇒ 1000 × `TimedOut`, 0 × `Allow` (Property-Test).
- `state_sequence_ask_allow`: Events in Reihenfolge `Received, Analyzed, Held, Decided(Allow), Forwarded, ResponseHeaders, Recorded`.
- `state_sequence_timeout`: `Received, Analyzed, Held, Decided(TimedOut), ResponseHeaders(403), Recorded`.
- `invalid_transition_blocks`: Erzwinge `Forwarded` ohne `Decided` ⇒ Flow wird mit `Block` beendet, `PROXY_005` im Stream.
- `broadcast_lag_maps`: Kapazität 8, 20 Events ohne Leser ⇒ Leser sieht `Lagged { n ≥ 12 }`.
- `queue_does_not_block_proxy`: 50 gleichzeitige Holds, ein weiterer Request an einen Stub mit sofortigem Allow wird in < 50 ms beantwortet.

### Akzeptanzkriterien
- [ ] Alle Tests grün, inklusive Property-Test `timeout_never_allows`. 27 Tests grün, `timeout_never_allows` belegt (1000 Holds, 0 Allow); `state_sequence_timeout` ist einen Schritt kürzer als spezifiziert, weil selbst erzeugte Block- und Timeout-Antworten kein `ResponseHeaders`-Ereignis auslösen (`handler.rs:1040` `record_block`).
- [ ] Ein Timeout produziert eine 403-Antwort mit `reason: timeout` beim Client (Integration mit HUM-015). Das Produkt antwortet 504, nicht 403 (Review-Korrektur 2026-09-02, CONVENTIONS 3.2: `Timeout 504`); `hold_timeout_returns_504` und `conf_19_curl_hold_timeout` belegen 504 mit `reason: timeout`. `HoldQueue::extend` aus der Spezifikation ist gebaut und getestet, aber von nichts erreichbar (HUM-109).
- [x] `FlowRegistry::list` liefert gehaltene Flows sortiert nach Deadline aufsteigend.

### Fallstricke
- `tokio::time::sleep_until` mit einer Deadline in der Vergangenheit löst sofort aus; bei `timeout_secs = 0` ist das die gewünschte „alles blocken"-Semantik für `ask_mode = none` (HUM-067), also nicht als Fehler behandeln.
- `DashMap`-Guards nicht über `.await` halten (Deadlock); `remove` vor dem Senden.
- `oneshot::Sender::send` schlägt fehl, wenn der Empfänger (die Proxy-Task) schon weg ist (Client hat Verbindung getrennt). Dann Entscheidung verwerfen, Flow auf `Block { reason: NoRoute }` setzen und Event senden, damit das UI den Eintrag aus der Queue nimmt. Client-Disconnect während Hold muss erkannt werden: hudsucker bricht die Task ab, der `hold`-Future wird gedroppt; im `Drop` eines Guards `transition(Decided(Block{NoRoute}))`.
- Broadcast-Kanal ist pro Registry, nicht pro Session; Filter im gRPC-Stream nach `session`.

### Referenzen
ADR-004; Pragmatiker-Review Punkt 6; tokio broadcast https://docs.rs/tokio/latest/tokio/sync/broadcast/index.html

---

## HUM-017 · Konformitäts-Matrix
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-015, HUM-016 · Blockiert: HUM-021

### Kontext
Risiko 2 aus BACKLOG.md 10: MITM-Randfälle sind unsere. Diese Matrix ist das Sicherheitsnetz, das in jedem CI-Lauf beweist, dass die Protokollfälle funktionieren, die ein Coding-Agent tatsächlich erzeugt (Bun-fetch, curl, git, pip, npm, SSE-Streams vom LLM).

### Ziel
Ein Fake-Upstream (`daemon/crates/proxy/tests/support/upstream.rs`, axum) mit definierten Endpunkten, plus ein Integrationstest-Modul, das jede Zeile der Matrix mit echten Clients (`curl`, `websocat`, `grpcurl`, `python3`, optional `node`, `git`) durch den Proxy fährt und das erwartete Verhalten prüft.

### Nicht-Ziel
Performance-Benchmarks. HTTP/3.

### Betroffene Pfade
- `daemon/crates/proxy/tests/support/upstream.rs` (neu), `tests/support/clients.rs` (neu: Wrapper, die prüfen ob ein Client installiert ist und den Test sonst als `skipped` markieren)
- `daemon/crates/proxy/tests/conformance.rs` (neu)
- `.github/workflows/ci.yml`: `apt install curl websocat grpcurl python3 git` im Job `rust-test`

### Spezifikation

Fake-Upstream, HTTP (Port A) und HTTPS mit selbstsigniertem Leaf (Port B, Test-CA getrennt von der Humanitl-CA, der Upstream-Connector des Proxys bekommt sie im Test als zusätzliche Root):

| Pfad | Verhalten |
|---|---|
| `GET /echo` | JSON mit Methode, Pfad, Headern, Body-Länge, Body-sha256 |
| `POST /echo` | dito |
| `GET /sse` | `text/event-stream`, 5 Events im Abstand `?interval_ms=` (Default 200), dann Ende |
| `GET /chunked` | `Transfer-Encoding: chunked`, 10 Chunks à 64 KB mit 50 ms Pause |
| `GET /big?mb=N` | N MB Nullen, `Content-Length` gesetzt |
| `POST /sink` | liest Body vollständig, antwortet mit Länge |
| `GET /ws` | WebSocket-Echo (axum `ws`) |
| `GET /redirect?to=` | 302 auf `to` |
| `GET /slow?ms=` | wartet ms, dann 200 |
| `GET /status/:code` | beliebiger Status |
| gRPC `Echo/Echo` | tonic-Service auf Port C (h2c) und hinter TLS auf Port B mit ALPN h2 |

Matrix (jede Zeile ein Test; Spalten Client, Szenario, Erwartung):

| # | Client | Szenario | Erwartung |
|---|---|---|---|
| 1 | curl | `GET http://…/echo` via Proxy | 200, Body-Echo korrekt, Flow `Recorded` |
| 2 | curl | `GET https://…/echo` via CONNECT | 200, Proxy-Leaf von Humanitl-CA, Flow hat `tls = true` |
| 3 | curl | `POST --data-binary @2mb https://…/echo` | Echo-sha256 stimmt, `decide` sah 2 MB |
| 4 | curl | `POST` 33 MB | 403 `body_cap`, Upstream nicht kontaktiert |
| 5 | curl | `-H 'Expect: 100-continue' POST 1mb` | 200, Upstream erst nach Allow |
| 6 | curl | `GET /sse` | erstes Event < 500 ms nach Allow, alle 5 Events, keine Pufferung |
| 7 | curl | `GET /chunked` | 640 KB, chunked erhalten oder re-chunked, Inhalt identisch |
| 8 | curl | `GET /big?mb=50` | 50 MB in < 10 s, Speicher des Daemons wächst < 64 MB (RSS-Messung im Test optional) |
| 9 | curl | `GET /redirect?to=https://other/echo` mit `-L` | zwei Flows, beide `Held`; zweiter hat anderen Host |
| 10 | curl | `--http2 GET https://…/echo` | funktioniert (Client↔Proxy h2 durch hudsucker), Upstream sieht h1 |
| 11 | websocat | `wss://…/ws` Echo „hello" | Upgrade-Request wird gehalten; nach Allow Echo funktioniert; Frames nicht gehalten |
| 12 | grpcurl | `-plaintext` Echo über Proxy (h2c) | Antwort korrekt, `grpc-status: 0` im Trailer |
| 13 | grpcurl | TLS Echo (`-cacert`) | Antwort korrekt (setzt `experimental.h2_upstream` oder Passthrough-Regel voraus; ohne Flag: Test erwartet dokumentierten Fehler `PROXY_006 h2 required`) |
| 14 | python3 urllib | `https://…/echo` mit Env-Kit | 200 ohne TLS-Fehler |
| 15 | python3 requests (falls installiert) | dito | 200 |
| 16 | git | `git ls-remote https://…/repo.git` gegen Fake, der `/info/refs` bedient | HTTP-Antwort kommt an (Inhalt egal), kein TLS-Fehler |
| 17 | node (falls installiert) | `fetch('https://…/echo')` | 200 |
| 18 | curl | Block durch Pipeline | 403, Body-Format, `Connection: close` |
| 19 | curl | Timeout 1 s | 403 `reason: timeout` nach ~1 s |
| 20 | curl | `GET http://[::1]:A/echo` | IPv6-Literal korrekt, `HostName::Ip` |
| 21 | curl | `GET http://127.0.0.1:A/echo` mit Authority nur im Host-Header (Proxy-Request mit relativer URI) | Authority aus Host-Header |
| 22 | curl | zwei Requests auf einer Keep-Alive-Verbindung, erster geblockt | zweiter Request kommt trotzdem als eigener Flow (oder Verbindung sauber geschlossen und neu) |

### Schritte
1. Fake-Upstream mit allen Endpunkten; eigenes Test-Zertifikat via `rcgen` zur Laufzeit.
2. `clients.rs`: `require("curl")` ⇒ Pfad oder `skip`.
3. Matrixzeilen 1–10, 18–22 mit curl.
4. 11–13 mit websocat/grpcurl; gRPC-Echo-Service in `tests/support/grpc_echo.rs` (tonic, kleines Proto im Testverzeichnis).
5. 14–17 mit Python/git/node.
6. CI: Pakete installieren, Job-Laufzeit < 5 min.

### Tests
Die Matrix ist die Testliste. Jeder Test heißt `conf_<nr>_<client>_<szenario>`.

### Akzeptanzkriterien
- [ ] Alle 22 Zeilen implementiert, keine als `ignored`; `skipped` nur bei fehlendem Client, und im CI sind alle Clients installiert. 22 Tests, keine `ignored`, CI installiert websocat und grpcurl; aber zwei Zeilen prüfen das Gegenteil der Matrix: Zeile 11 assertet 426 statt eines gehaltenen und dann durchgereichten Upgrades (`conformance.rs:629`, HUM-110), Zeile 12 assertet das Fehlen von `grpc-status: 0` (sanktioniert in CONVENTIONS 4.10); übersprungene Zeilen sind lokal unsichtbar, weil `clients::skip` über `eprintln!` geht.
- [x] Laufzeit des Moduls < 3 min lokal. (3,8 s)
- [ ] Zeile 13 dokumentiert das h2-Verhalten in `docs/SECURITY.md` („gRPC über TLS braucht `experimental.h2_upstream`"). Der Absatz steht (`docs/SECURITY.md:438-441`), beschreibt aber ein Signal, das es nicht gibt: kein Code baut `PROXY_007`, der Client sieht einen nackten TLS-Alert `no application protocol` (HUM-108).

### Fallstricke
- axum `ws` und hudsucker: WebSocket über CONNECT erfordert, dass hudsucker den Upgrade durchreicht; `WebSocketHandler` von hudsucker nur registrieren, wenn Frames beobachtet werden sollen (HUM-026), sonst Default-Passthrough.
- `grpcurl -plaintext` über einen HTTP-Proxy: grpcurl respektiert `HTTP_PROXY` nur für TLS-Verbindungen inkonsistent; im Test das Proxy-Ziel direkt mit `-H` und `--proxy` prüfen, ggf. `HTTPS_PROXY` setzen und TLS-Variante verwenden.
- Tests laufen parallel; jeder Test startet eigenen Fake-Upstream und eigene Proxy-Session auf ephemeren Ports.
- `curl --http2` gegen den Proxy: hudsucker verhandelt h2 zum Client nur mit Feature `http2`; ohne Feature fällt curl auf h1 zurück, Test würde falsch grün. Aushandlung explizit prüfen (`curl -w '%{http_version}'`).

### Referenzen
BACKLOG.md 10 Risiko 2; Proxy-Recherche Abschnitt 1 (SSE-Puffer-Problem in mitmproxy als Warnung); axum ws https://docs.rs/axum/latest/axum/extract/ws/index.html ; grpcurl https://github.com/fullstorydev/grpcurl ; websocat https://github.com/vi/websocat

---

## HUM-018 · gRPC-Server Grundgerüst
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-003, HUM-016 · Blockiert: HUM-064, HUM-019, HUM-021

### Kontext
ADR-003. Der Daemon ist ein gRPC-Server auf einem Unix-Socket; UI und CLI sind Clients. Dieses Issue liefert die minimale Oberfläche, die Sprint 1 braucht: `GetInfo`, `Subscribe`, `Decide`, `ListFlows` (in-memory). Alle weiteren RPCs bleiben `UNIMPLEMENTED` mit klarer Fehlermeldung.

### Ziel
`humanitl_ipc::serve(paths, registry, queue, config)` startet tonic auf `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600), prüft auf jedem Aufruf das Token aus `x-humanitl-token`, streamt `FlowEvent`s pro Session mit Lag-Umwandlung, und nimmt Entscheidungen entgegen. `humanitld` verdrahtet Proxy, Registry, Queue und IPC.

### Nicht-Ziel
`Rules`, `Sandbox`, `Terminal`, `Audit`, `GetBody`-RPCs (Sprint 2/3), Persistenz.

### Betroffene Pfade
- `daemon/crates/ipc/Cargo.toml`: `tonic`, `prost`, `tonic-build` (build.rs mit `proto/humanitl/v1/humanitl.proto`), `tokio-stream`
- `daemon/crates/ipc/build.rs` (neu), `src/lib.rs`, `src/server.rs` (neu), `src/convert.rs` (neu: Domain ⇄ Proto), `src/auth.rs` (neu), `src/diag.rs` (`IPC_001..`)
- `daemon/bin/humanitld/src/main.rs`: Verdrahtung, `tracing`-Init (JSON nach stderr, journald übernimmt), Signal-Handling (SIGTERM ⇒ Sessions stoppen, Socket löschen)

### Spezifikation

**Token (`auth.rs`).** Beim Start: 32 Zufallsbytes, hex, in `token`-Datei (0600) schreiben, überschreiben falls vorhanden. `tonic::service::Interceptor`: Metadata `x-humanitl-token` muss byteweise gleich sein (`subtle::ConstantTimeEq`), sonst `Status::unauthenticated("missing or invalid token")`. `GetInfo` ist ebenfalls geschützt (kein unauthentifizierter Endpunkt).

**Server (`server.rs`).**

```rust
pub struct IpcServer { registry: Arc<FlowRegistry>, queue: Arc<HoldQueue>, info: Info }
#[tonic::async_trait]
impl humanitl::humanitl_server::Humanitl for IpcServer {
    async fn get_info(&self, _: Request<Empty>) -> Result<Response<Info>, Status>;
    // Info { daemon_version: env!("CARGO_PKG_VERSION"), proto_version: "1.0", capabilities: ["hold","bwrap"] }
    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<FlowEvent, Status>> + Send>>;
    async fn subscribe(&self, req: Request<SubscribeRequest>) -> Result<Response<Self::SubscribeStream>, Status>;
    // SubscribeRequest { session_id: optional string, since_seq: optional uint64 }
    // Stream: BroadcastStream(registry.subscribe()) → filter session → map Lagged(n) zu FlowEvent::Lagged{n}
    async fn decide(&self, req: Request<DecideRequest>) -> Result<Response<DecideResponse>, Status>;
    // DecideRequest { flow_id, decision: oneof { Allow{}, AllowEdited{ HttpRequest }, Block{ reason_note } }, remember: optional Rule (ignoriert bis HUM-027, aber akzeptiert) }
    // NotHeld ⇒ Status::failed_precondition("flow not held")
    async fn list_flows(&self, req: Request<ListFlowsRequest>) -> Result<Response<FlowPage>, Status>;
    // in-memory aus Registry; Filter: session, state; Sort: created desc; limit ≤ 500
    // alle anderen RPCs: Status::unimplemented("<name> arrives in HUM-0xx")
}
pub async fn serve(socket: PathBuf, token: PathBuf, server: IpcServer, shutdown: impl Future) -> Result<(), Diagnostic>;
// UnixListener::bind, chmod 0600, tonic Server::builder().add_service(HumanitlServer::with_interceptor(server, auth)).serve_with_incoming_shutdown(UnixListenerStream, shutdown)
```

**Konvertierung (`convert.rs`).** `From<core::FlowEvent> for proto::FlowEvent` usw. Bodies nie inline in Events; `HttpRequest` im `Held`-Event enthält `BodyRef` und `body_preview` (erste `min(size, 64 KiB)` Bytes, nur wenn `inline` vorhanden). `AllowEdited` aus Proto: Body kommt als `bytes` (≤ `hold.body_cap_bytes`, sonst `Status::invalid_argument`).

**Daemon-Main.** Reihenfolge: Paths ⇒ Config laden (HUM-062) ⇒ `tracing` ⇒ `CaStore` ⇒ `FlowRegistry`, `HoldQueue` ⇒ `ProxyCore` ⇒ IPC-Server ⇒ auf SIGTERM/SIGINT warten ⇒ Sessions stoppen ⇒ Socket und Token löschen. Diagnostics beim Start als JSON auf stderr und Exit 1.

Diagnostic-Codes: `IPC_001` Socket-Bind fehlgeschlagen (fix: `CopyCommand("rm $XDG_RUNTIME_DIR/humanitl/daemon.sock")` wenn Datei existiert und kein Daemon läuft; Prüfung per Connect-Versuch), `IPC_002` Token-Datei nicht schreibbar, `DAEMON_001` bereits ein Daemon aktiv (Connect gelingt).

### Schritte
1. `build.rs` mit `tonic_build::configure().build_server(true).build_client(true)`; Rust-Codegen in `OUT_DIR`, Dart-Codegen bleibt CI-Sache (HUM-003).
2. `convert.rs` mit Round-Trip-Tests.
3. `auth.rs`, `server.rs` mit `GetInfo`, `Subscribe`, `Decide`, `ListFlows`.
4. `humanitld` Main mit Verdrahtung und Shutdown.
5. Client-Helfer `humanitl_ipc::client::connect(paths) -> HumanitlClient<Channel>` über `tonic::transport::Endpoint::from_static("http://[::]:50051").connect_with_connector(service_fn(|_| UnixStream::connect(path)))` mit Token-Interceptor; wird von CLI (HUM-064) und Tests genutzt.

### Tests
- `get_info_requires_token`: ohne Token ⇒ `Unauthenticated`; mit falschem ⇒ `Unauthenticated`; richtig ⇒ `Info`.
- `subscribe_receives_events`: Session starten, Flow via Proxy ⇒ Stream liefert `Received`, `Analyzed`, `Held` innerhalb 1 s.
- `subscribe_filters_session`: zwei Sessions, Stream für A sieht keine Events von B.
- `decide_allow_roundtrip`: `Held`-Event ⇒ `Decide(Allow)` ⇒ Client-curl bekommt 200; Stream zeigt `Decided`, `Forwarded`.
- `decide_not_held`: fremde ID ⇒ `FailedPrecondition`.
- `decide_edited_too_big`: Body > cap ⇒ `InvalidArgument`.
- `lagged_event`: Event-Buffer 8, Stream-Konsument pausiert, 20 Flows ⇒ `Lagged{n}` im Stream, danach `ListFlows` liefert alle 20.
- `socket_perms`: `daemon.sock` 0600.
- `second_daemon_refuses`: zweiter `humanitld` ⇒ `DAEMON_001`, Exit 1, erster läuft weiter.
- `shutdown_cleans_up`: SIGTERM ⇒ Socket und Token gelöscht, Sandbox-Sessions beendet (bwrap-Prozess weg).

### Akzeptanzkriterien
- [ ] `grpcurl -unix -H 'x-humanitl-token: …' -plaintext $XDG_RUNTIME_DIR/humanitl/daemon.sock humanitl.v1.Humanitl/GetInfo` liefert Versionen (grpcurl unterstützt `-unix`). So nicht ausführbar: ohne `-proto`/`-protoset` braucht grpcurl Server-Reflection, und `tonic-reflection` steht in keinem Manifest; mit `-protoset proto/descriptor.binpb` geht es; die Fähigkeit belegt `daemon_end_to_end.rs:137-147` über den Socket.
- [x] Alle Tests grün; `cargo doc` ohne Warnungen für `humanitl-ipc`. (170 Tests in `humanitl-ipc`; `subscribe_filters_session` entfällt bewusst, CONVENTIONS 4.12)
- [ ] `humanitld` startet in < 500 ms und loggt eine JSON-Zeile `{"level":"info","msg":"listening","socket":…}`. Start in 0,42 s gemessen; die Zeile hat die Form von `tracing_subscriber::fmt().json()` (`level` in Großbuchstaben, Text unter `fields.message`, kein `msg`), und nichts liest die spezifizierte Form.

### Fallstricke
- tonic über UDS: die Client-`Endpoint`-URI ist ein Platzhalter; der Connector ignoriert sie. Nicht versuchen, `unix://` in die URI zu schreiben.
- `BroadcastStream` liefert `Err(Lagged)`; die Umwandlung in ein reguläres Event darf den Stream nicht beenden (`filter_map` statt `?`).
- `serve_with_incoming` benötigt `tokio_stream::wrappers::UnixListenerStream`; Verbindungen liefern `tonic::transport::server::Connected` für `UnixStream` ab tonic 0.10, sonst Wrapper.
- Token-Datei vor dem Socket schreiben, sonst Race für Clients, die auf den Socket warten.
- `prost`-Enums beginnen mit `_UNSPECIFIED = 0`; `From`-Implementierungen müssen `Unspecified` explizit als Fehler behandeln (`Status::invalid_argument`).

### Referenzen
ADR-003; CONVENTIONS 3.6; tonic UDS-Beispiel https://github.com/hyperium/tonic/tree/master/examples/src/uds ; grpc-dart UDS https://github.com/grpc/grpc-dart/issues/299

---

## HUM-064 · CLI-Grundgerüst
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-062, HUM-018 · Blockiert: HUM-021, HUM-065, HUM-067

### Kontext
ADR-013 (CLI ist erstklassig) und ADR-011 (Flags aus dem Config-Schema). Ab jetzt nutzen Escape-Tests und Demo-Skripte die CLI, nicht Ad-hoc-Skripte, damit dieselben Codepfade wie später beim Nutzer laufen.

### Ziel
Binary `humanitl` mit `clap` und den Subkommandos `sandbox run|argv|check`, `daemon status`, `flows list|show` (minimal), `config get|schema`. `sandbox run` startet über den laufenden Daemon eine Session mit dem angegebenen Kommando als Agent, reicht stdin/stdout/stderr durch, leitet `SIGINT` weiter und endet mit dem Exit-Code des Kommandos. Exit-Codes gemäß CONVENTIONS 3.8. Diagnostics werden als lesbarer Block gerendert.

### Nicht-Ziel
`run` mit Agent-Adapter und `--ask terminal` (HUM-067), `rules`, `audit`, `daemon install` (HUM-065, HUM-070), Terminal-PTY-Durchreichung (HUM-042; hier reicht `inherit`, was für nicht-interaktive Kommandos genügt).

### Betroffene Pfade
- `daemon/bin/humanitl/Cargo.toml`, `src/main.rs`, `src/cli.rs` (clap-Struktur), `src/render.rs` (Diagnostic- und Tabellen-Ausgabe), `src/cmd/{sandbox,daemon,flows,config}.rs`
- `daemon/crates/ipc/src/client.rs` (aus HUM-018)
- Proto/Server: `Sandbox`-RPC in minimaler Form (`Start { work_dir, work_mode, profile, agent_argv }`, `Stop`, `Status`) wird hier ergänzt, weil `sandbox run` ihn braucht; volle Form in HUM-040

### Spezifikation

```rust
#[derive(Parser)]
#[command(name = "humanitl", version, about = "Human-in-the-loop network moderation for AI agents")]
struct Cli {
    #[command(flatten)] global: GlobalOpts,     // --json, --config PATH, --profile NAME, -v/-q
    #[command(subcommand)] cmd: Cmd,
}
#[derive(Subcommand)]
enum Cmd {
    Run(RunArgs),                 // Platzhalter: "arrives in HUM-067", Exit 1
    Sandbox { #[command(subcommand)] cmd: SandboxCmd },
    Rules { .. },                 // Platzhalter, HUM-065
    Flows { #[command(subcommand)] cmd: FlowsCmd },
    Audit { .. },                 // Platzhalter, HUM-070
    Config { #[command(subcommand)] cmd: ConfigCmd },
    Daemon { #[command(subcommand)] cmd: DaemonCmd },
}
enum SandboxCmd { Run { #[arg(long)] work: Option<PathBuf>, #[arg(long, default_value="rw")] work_mode: WorkMode, #[arg(last = true)] cmd: Vec<OsString> }, Argv, Check }
enum FlowsCmd { List { filter: Option<String> }, Show { id: String } }
enum ConfigCmd { Get { key: String }, Schema }
enum DaemonCmd { Status }
```

Config-Flags: `humanitl-config` liefert `clap::Command`-Argumente generiert aus dem Schema (`--hold-timeout-secs`, `--llm-endpoint`, …) über `Config::clap_args()`; Werte werden mit Präzedenz CLI > Env > Profil > Datei > Default aufgelöst (`Config::resolve(sources)`).

Verhalten `sandbox run`:
1. Daemon-Verbindung (`client::connect`); Fehler ⇒ Diagnostic `DAEMON_001` „Daemon nicht erreichbar" mit fix `CopyCommand("systemctl --user start humanitld")`, Exit 2.
2. `Sandbox(Start { work_dir: cwd oder --work, … , agent_argv: cmd })` ⇒ `SandboxEvent`-Stream: `Started { sandbox_id, argv_display }`, `Check { results }`, `Output { bytes }` (stdout/stderr des Agenten, bis HUM-042 ohne PTY), `Exited { code }`.
3. Ist ein `Check` mit `passed = false` dabei ⇒ Diagnostic anzeigen, Exit 3; der Daemon startet den Agenten in diesem Fall nicht (Server-seitig: Check läuft vor Agent-Start, Pflicht).
4. `SIGINT` ⇒ `Sandbox(Stop)`; Exit mit `Exited.code`.

`sandbox argv`: gibt `argv_display` für das aktuelle Profil und cwd aus, ohne zu starten (`Sandbox(Plan)`-RPC, nur Argv). `sandbox check`: startet eine kurzlebige Sandbox nach demselben Plan, liest die Prüfzeilen des Shims und rendert die drei Prüfungen als Tabelle mit ✓/✗ und `evidence`.

`render.rs`: Diagnostic-Format auf stderr:

```
error[SANDBOX_003]: Unprivilegierte User-Namespaces sind deaktiviert
  why: bwrap: setting up uid map: Permission denied
  fix: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
  docs: https://…
```

Mit `--json`: Diagnostic als JSON auf stdout, eine Zeile.

### Schritte
1. clap-Struktur, `--json`, Renderer.
2. `daemon status` (GetInfo) und `config get|schema`.
3. `Sandbox`-RPC minimal in Proto/Server (Start/Stop/Plan/Status als `oneof` in `SandboxRequest`, Stream `SandboxEvent`), Server-seitig: Check vor Agent-Start erzwingen.
4. `sandbox run|argv|check`.
5. `flows list|show` gegen `ListFlows`/`GetFlow` (GetFlow minimal: Summary + Header, kein Body).
6. Escape-Harness (HUM-006) auf `humanitl sandbox run --profile test -- /tests/escape/esc-N.sh` umstellen.

### Tests
- `cli_help_snapshot`: `humanitl --help` und je Subkommando, Snapshot.
- `cli_json_diagnostic`: Daemon aus ⇒ `humanitl --json daemon status` ⇒ JSON mit `code: "DAEMON_001"`, Exit 2 (im Register ist `DAEMON_002` die Proto-Version).
- `sandbox_run_exit_code` (Integration): `humanitl sandbox run -- sh -c 'exit 5'` ⇒ Exit 5.
- `sandbox_run_check_fails_blocks_start`: Daemon mit `--test-fail-check` (Test-Flag) ⇒ Exit 3, Agent nie gestartet (Marker-Datei fehlt).
- `sandbox_argv_matches_plan`: Ausgabe gleicht `LaunchPlan.argv_display` aus HUM-011-Snapshot.
- `sigint_stops_sandbox`: `sandbox run -- sleep 60`, SIGINT an CLI ⇒ bwrap-Prozess in < 2 s weg, Exit 130.
- `config_precedence`: `HUMANITL_HOLD__TIMEOUT_SECS=7 humanitl --hold-timeout-secs 9 config get hold.timeout_secs` ⇒ `9`; ohne Flag ⇒ `7`.

### Akzeptanzkriterien
- [x] `humanitl sandbox check` zeigt drei grüne Zeilen auf dem CI-Runner.
- [ ] `humanitl sandbox run -- curl -sS https://example.com` gibt nach Freigabe über gRPC (Test-Client) die Seite aus; ohne Freigabe nach Timeout den 403-Text. Belegt für plain HTTP im M1-Lauf (`m1_sealed_box.sh:189,214`); `https://` durch `sandbox run` prüft kein Test (nur `conf_02` auf Crate-Ebene), und der unentschiedene Request bekommt 504, nicht 403 (CONVENTIONS 4.11).
- [x] Exit-Codes 0/1/2/3 sind in `--help` dokumentiert und getestet. (Exit 3 nur als Mapping-Unit-Test, kein Lauf erreicht ihn)
- [x] Alle Config-Schlüssel aus CONVENTIONS 3.7 sind als Flags in `--help` sichtbar. (drei unter ihren Namen aus CONVENTIONS 4.4)

### Fallstricke
- `#[arg(last = true)]` für `-- CMD...` nötig, sonst frisst clap Flags des Agenten.
- Der Agent-Output kommt über gRPC-Stream; bei nicht-interaktiven Kommandos ausreichend, aber Reihenfolge stdout/stderr ist nicht garantiert. Im Test nur stdout prüfen.
- `ctrlc`-Crate oder `tokio::signal::ctrl_c`; nach `Stop` auf `Exited` warten, nicht sofort beenden.
- `--profile` ist global und muss vor dem Subkommando erlaubt sein (`global = true`).
- Exit-Code > 255 gibt es nicht; Signal-Tod ⇒ 128+n.

### Referenzen
ADR-011, ADR-013; CONVENTIONS 3.7, 3.8; clap https://docs.rs/clap/latest/clap/_derive/index.html

---

## HUM-019 · Flutter-Shell
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-003, HUM-005, HUM-008 · Blockiert: HUM-020

### Kontext
Der Rahmen, in dem alle Screens leben: Fenster, Icon-Rail, Command Palette, Statusleiste, Theme, Verbindung zum Daemon mit Versionscheck, Setup-Screen als Fallback. Design nach BACKLOG.md Abschnitt 5 („Airlock").

### Ziel
`flutter run -d linux` zeigt ein Fenster mit linker Icon-Rail (Intercept, History, Rules, Sandbox, Audit; `Ctrl+1..5`), Header 40 px, Statusleiste 24 px, Command Palette (`Ctrl+K`) mit Navigationsbefehlen, Dark-Theme aus `HTokens`, Light-Theme umschaltbar. Beim Start verbindet die App mit dem Daemon (`GrpcDaemonClient`) oder mit dem Fake (`--dart-define=HUMANITL_FAKE=1` oder Flag `--fake`), ruft `GetInfo`, prüft `proto_version`, und zeigt bei Fehler den Setup-Screen mit Diagnostic statt einer leeren Queue.

### Nicht-Ziel
Inhalt der Screens (HUM-020 ff.), Tray und Notifications (HUM-034), Setup-Checkliste mit Aktionen (HUM-044; hier nur Platzhalter mit Diagnostic und „Erneut verbinden").

### Betroffene Pfade
- `app/lib/main.dart`, `app/lib/app.dart`
- `app/lib/core/ipc/daemon_client.dart` (Interface), `grpc_daemon_client.dart`, `fake_daemon_client.dart` (aus HUM-005 hierher), `daemon_paths.dart`
- `app/lib/core/domain/*.dart` (freezed): `Flow`, `FlowState`, `Decision`, `Diagnostic`, `DaemonInfo`
- `app/lib/features/shell/shell_screen.dart`, `widgets/icon_rail.dart`, `widgets/header_bar.dart`, `widgets/status_bar.dart`, `widgets/command_palette.dart`, `providers/navigation.dart`, `providers/connection.dart`
- `app/lib/features/setup/setup_screen.dart` (Platzhalter)
- `app/packages/ui/` (aus HUM-008), `app/l10n/app_en.arb`, `app_de.arb`
- `app/linux/runner/my_application.cc`: Fenstertitel, Mindestgröße 1100×700

### Spezifikation

```dart
abstract class DaemonClient {
  Future<DaemonInfo> getInfo();
  Stream<FlowEvent> subscribe({String? sessionId});
  Future<void> decide(FlowId id, Decision decision, {Rule? remember});
  Future<FlowPage> listFlows(FlowFilter filter, {int limit = 200, String? cursor});
  Future<FlowDetail> getFlow(FlowId id);
  Stream<Uint8List> getBody(BodyRef ref);
  Future<void> close();
}
```

`GrpcDaemonClient`: `ClientChannel(InternetAddress(path, type: InternetAddressType.unix), port: 0, options: ChannelOptions(credentials: ChannelCredentials.insecure()))`, Token aus `$XDG_RUNTIME_DIR/humanitl/token` als `CallOptions(metadata: {'x-humanitl-token': token})` auf jedem Call. Fehler werden in `Diagnostic` übersetzt: `UNAVAILABLE` ⇒ `DAEMON_002`, `UNAUTHENTICATED` ⇒ `DAEMON_005` („Token stimmt nicht, Daemon neu gestartet?" fix: App neu verbinden), Versions-Mismatch ⇒ `DAEMON_006`.

Provider (CONVENTIONS 3.9): `daemonClientProvider` (wählt Grpc/Fake nach Flag), `daemonInfoProvider` (FutureProvider, retry über `ref.invalidate`), `connectionStateProvider` (Notifier: `connecting | connected(info) | failed(diagnostic)`), `navigationProvider` (Notifier<Section>), `themeModeProvider`.

Widget-Baum:

```
HumanitlApp (ShadcnApp aus packages/ui, theme: HTokens.dark/light, localizationsDelegates)
└─ ConnectionGate (switch connectionState)
   ├─ failed  → SetupScreen(diagnostic, onRetry)
   ├─ connecting → HSplash (Logo, „Verbinde…")
   └─ connected → ShellScreen
        └─ Column
           ├─ HeaderBar (40 px): [Logo] [Section-Titel] ··· [Intercept ON/OFF Pill (statisch bis HUM-028)] [Hold-Count Badge] [Isolation-Ring Platzhalter grau] [Ctrl+K Button]
           ├─ Row
           │  ├─ IconRail (48 px): 5 Einträge, Tooltip, aktiver Eintrag mit Akzent-Rail links
           │  └─ Expanded: IndexedStack(children: [InterceptScreen, HistoryScreen, RulesScreen, SandboxScreen, AuditScreen]) — Platzhalter-Screens mit Titel
           └─ StatusBar (24 px): Daemon-Version, Session-ID, Verbindung ●
        + CommandPalette als Overlay (shadcn `Command`), Einträge: „Go to Intercept/History/…", „Toggle theme", „Reconnect"
```

Shortcuts als `Shortcuts`/`Actions` auf `ShellScreen`-Ebene mit den `Intent`-Klassen aus CONVENTIONS 3.9: `NavIntent(n)`, `PaletteIntent`. Wichtig: `Shortcuts` mit `SingleActivator` und ohne Textfeld-Fokus-Kollision: Einzeltasten-Shortcuts (`A`, `B`, `J`…) werden erst in HUM-020 ergänzt und nur aktiviert, wenn `FocusManager.instance.primaryFocus` kein `EditableText` ist (Helfer `isTextInputFocused()` in `packages/ui`).

l10n-Schlüssel: `shell_nav_intercept`, `shell_nav_history`, `shell_nav_rules`, `shell_nav_sandbox`, `shell_nav_audit`, `shell_palette_hint`, `setup_title`, `setup_retry`, `setup_daemon_missing_title` usw.; Deutsch nach BACKLOG.md 5 (Terminologie).

### Schritte
1. `pubspec.yaml` mit gepinnten Paketen (CONVENTIONS 3.9), `build_runner` läuft, `flutter analyze` sauber.
2. Domain-Modelle (freezed) und Proto-Konvertierung (`core/ipc/convert.dart`).
3. `DaemonClient`-Interface, `FakeDaemonClient` (übernommen), `GrpcDaemonClient` mit UDS und Token.
4. Provider und `ConnectionGate`.
5. `ShellScreen` mit Rail, Header, Statusleiste, `IndexedStack`, Command Palette, Theme-Toggle.
6. Setup-Platzhalter mit Diagnostic-Rendering (`HDiagnosticCard` in `packages/ui`: Titel, why, fix-Button, docs-Link).
7. Fenster-Mindestgröße, Titel „Humanitl".

### Tests
- Widget: `shell_renders_rail_and_sections`: fünf Rail-Einträge, `Ctrl+2` wechselt auf History (Titel im Header ändert sich).
- Widget: `palette_opens_and_navigates`: `Ctrl+K`, Eingabe „hist", Enter ⇒ History.
- Widget: `connection_failed_shows_setup`: `FakeDaemonClient` mit `getInfo` ⇒ throws `DAEMON_002` ⇒ `SetupScreen` sichtbar mit Code.
- Widget: `version_mismatch_shows_setup`: `proto_version = "2.0"` ⇒ `DAEMON_006`.
- Unit: `grpc_client_translates_status`: `GrpcError(UNAVAILABLE)` ⇒ `Diagnostic.code == DAEMON_002`.
- Golden (alchemist, CI-Modus): `shell_dark`, `shell_light` bei 1280×800.
- Integration (`integration_test/shell_test.dart`, xvfb): App mit `HUMANITL_FAKE=1` startet, Rail sichtbar, keine Exceptions in 3 s.

### Akzeptanzkriterien
- [ ] `flutter run -d linux --dart-define=HUMANITL_FAKE=1` zeigt die Shell in < 2 s nach Fensteröffnung. Der Pfad steht (`LaunchOptions`, `HUMANITL_FAKE=1` gegen `humanitld --fake`), die 2 s misst kein Test; der einzige Messwert hier (12 bis 14 s bis zum ersten RPC aus dem Debug-Bundle unter Last) bestätigt und widerlegt nichts.
- [ ] Gegen echten Daemon (HUM-018): Statusleiste zeigt dessen Version; Daemon stoppen ⇒ innerhalb 5 s Setup-Screen mit `DAEMON_002`; Daemon starten, „Erneut verbinden" ⇒ Shell. Heartbeat, Statusleiste und Setup-Screen sind gebaut und gegen den Dart-Fake getestet (`setup_test.dart:65`); der Code ist `DAEMON_001`, nicht `DAEMON_002` (im Register ist `DAEMON_002` der Proto-Mismatch), und kein automatisierter Test verbindet die App mit einem laufenden `humanitld`.
- [x] Alle sichtbaren Strings kommen aus ARB (Lint: kein String-Literal in `Text(...)` außerhalb `packages/ui`-Galerie; Custom-Lint oder Review-Checkliste). (0 Literale in `app/lib`; ein Lint oder eine Checkliste existiert nicht, die Regel hält durch Disziplin)
- [x] Goldens identisch auf CI und lokal (CI-Modus).

### Fallstricke
- `CallbackShortcuts`/`Shortcuts` fangen Tasten auch in Textfeldern; deshalb der Fokus-Check. Für `Ctrl+K` ist das unkritisch, für Einzeltasten später zwingend.
- `InternetAddress(path, type: unix)` benötigt `port: 0` im `ClientChannel`; sonst versucht grpc-dart TCP.
- grpc-dart hält den Stream bei Daemon-Neustart nicht; `subscribe` muss bei `UNAVAILABLE` mit Backoff neu verbinden (Provider-Logik in HUM-020, hier nur `connectionState`).
- shadcn_flutter 0.0.54 ohne Material: keine `MaterialApp`, keine `Scaffold`; Theme über `ShadcnApp`. Wenn ein Paket `Material` verlangt (`flutter_local_notifications` nicht, `re_editor` nicht), `shadcn_flutter_material` als Brücke.
- Impeller auf NVIDIA/Wayland: falls Rendering-Artefakte, `flutter run --no-enable-impeller` dokumentieren; nicht im Code umgehen.
- `window_manager` Mindestgröße nur nach `ensureInitialized`.

### Referenzen
BACKLOG.md 5 (IA, Tokens), ADR-009; CONVENTIONS 3.9; shadcn_flutter https://pub.dev/packages/shadcn_flutter ; grpc-dart UDS https://github.com/grpc/grpc-dart/issues/299 ; riverpod https://riverpod.dev

---

## HUM-020 · Intercept-Screen v1
Sprint: 1 · Größe: L · Abhängigkeiten: HUM-019 · Blockiert: HUM-028, HUM-029, HUM-030

### Kontext
Das Herzstück. Version 1 zeigt gehaltene Requests aus dem Event-Stream in drei Panes, mit Karte, Countdown und den Aktionen Allow/Block, gegen den Fake-Daemon und gegen den echten Daemon. Die vollständige Aktionsleiste (Release Valve, Merken, Scope) kommt in HUM-028, Body-Ansichten in HUM-030, Gruppierung in HUM-029.

### Ziel
`InterceptScreen` mit `ResizablePanel` 28/44/28 (Mindestbreiten 280/480/260): links die Queue (`heldFlowsProvider`, sortiert nach Deadline), Mitte die Request-Karte des ausgewählten Flows (Header-Zeile, Sektionen Query/Headers/Body-Raw als `Collapsible`, Countdown-Ring), rechts ein Domain-Panel-Platzhalter (Host, PSL-Apex, „nicht im Katalog"), unten die Aktionsleiste mit Allow (Enter/`A`/Ctrl+F) und Block (`B`/Ctrl+L). Neue Flows erscheinen mit Slide+Fade, entschiedene verlassen die Queue mit Collapse und Richtung. Timeout markiert die Karte als „Angehalten, dann blockiert (Timeout)" und lässt sie 3 s stehen.

### Nicht-Ziel
Edit+Allow, Merken/Scope-Raster, Gruppierung, Batch, JSON-Tree, Findings-Markierung, Katalog-Karte, Notifications. Alles Sprint 2.

### Betroffene Pfade
- `app/lib/features/intercept/intercept_screen.dart`
- `app/lib/features/intercept/providers/flows.dart` (`flowEventsProvider`, `flowsProvider`, `heldFlowsProvider`, `selectedFlowIdProvider`)
- `app/lib/features/intercept/widgets/queue_pane.dart`, `queue_row.dart`, `request_card.dart`, `section_headers.dart`, `section_query.dart`, `section_body_raw.dart`, `countdown_ring.dart`, `action_bar.dart`, `domain_pane_placeholder.dart`
- `app/lib/core/domain/flow.dart`: `Flow` mit `copyWith` für Events
- `app/packages/ui/`: `HCollapsible`, `HMethodBadge`, `HStateDot`, `HCountdownRing`
- `app/test/features/intercept/*`, `app/test/goldens/intercept/*`

### Spezifikation

**Provider.**

```dart
@riverpod
Stream<FlowEvent> flowEvents(Ref ref) => ref.watch(daemonClientProvider).subscribe();
// mit Reconnect: bei Fehler 1 s, 2 s, 4 s … max 30 s, dann ListFlows(state: held) zum Nachladen

@riverpod
class Flows extends _$Flows {   // Map<FlowId, Flow>
  @override Map<FlowId, Flow> build() { ref.listen(flowEventsProvider, (_, next) => next.whenData(_apply)); return {}; }
  void _apply(FlowEvent e) => switch (e) {
    Received(:final flow) => state = {...state, flow.id: flow},
    Analyzed(:final id, :final findings) => _update(id, (f) => f.copyWith(findings: findings)),
    Held(:final id, :final deadline) => _update(id, (f) => f.copyWith(state: FlowState.held, deadline: deadline)),
    Decided(:final id, :final decision) => _update(id, (f) => f.copyWith(state: FlowState.decided, decision: decision, decidedAt: DateTime.now())),
    Forwarded(:final id) => _update(id, (f) => f.copyWith(state: FlowState.forwarded)),
    ResponseHeaders(:final id, :final status) => _update(id, (f) => f.copyWith(status: status)),
    Recorded(:final id) => _update(id, (f) => f.copyWith(state: FlowState.recorded)),
    TimedOut(:final id) => _update(id, (f) => f.copyWith(state: FlowState.decided, decision: const Decision.timedOut())),
    Lagged(:final n) => _resync(),
    ResponseChunk() => state,   // v1 ignoriert Chunks
  };
}

@riverpod
List<Flow> heldFlows(Ref ref) => ref.watch(flowsProvider).values.where((f) => f.state == FlowState.held).sortedBy((f) => f.deadline!).toList();
// plus „recently decided" (decidedAt < 3 s) für die Exit-Animation: `visibleQueueFlowsProvider`
```

Auswahl: `selectedFlowIdProvider` (Notifier<FlowId?>). Regel: Neue Flows stehlen nie die Auswahl. Ist nichts ausgewählt und ein Flow kommt, wird er ausgewählt. Verlässt der ausgewählte Flow die Queue, wird der nächste in Deadline-Reihenfolge ausgewählt (nicht der neueste).

**Queue-Zeile (36 px, selektiert 56 px).** Von links: Zustands-Rail 4 px (amber, selektiert Akzent 2 px), `HMethodBadge`, Host 13/500 `fg-0`, Pfad Mono 12 `fg-1` mittig gekürzt (`TextOverflow.ellipsis` reicht nicht; eigene Funktion `middleEllipsis(text, maxChars)`), rechts Countdown `mm:ss` Mono 11, Findings-Chip (leer in v1). Zweitzeile bei Selektion: Größe, Content-Type, „angehalten seit 00:12". Hover: `bg-3`, Ghost-Buttons Allow/Block rechts.

**Request-Karte.** Kopf: `HMethodBadge`, vollständige URL Mono 13 (selektierbar), Zustands-Dot, `HCountdownRing` 20 px neben `mm:ss`. Darunter `HCollapsible`-Sektionen: „Query (n)" (Tabelle Key/Value, Mono), „Headers (n)" (Tabelle, `Authorization`/`Cookie`/`X-Api-Key`-Werte standardmäßig maskiert `••••` mit Auge-Toggle), „Body (size, content-type)" (Raw: `SelectableText` Mono 12 aus `body_preview`; > 64 KB: Hinweis „Vorschau der ersten 64 KB"). Alles read-only.

**Countdown-Ring.** `CustomPainter`, Kreis 20 px, Strich 2 px, Farbe `held`, Fortschritt `remaining / total`; unter 20 % zusätzlich Opazitäts-Puls 0,6↔1,0 mit 1,2 s Periode (kein Farbwechsel). Ticker 250 ms über `Ticker`, nicht `Timer` pro Zeile (ein gemeinsamer `nowProvider`, der jede 250 ms `DateTime.now()` liefert; alle Rings hören darauf).

**Aktionsleiste (v1).** Rechts: `HButton.primary` „Allow" (Enter), links davon mit 24 px Abstand `HButton.destructive` „Block" (`B`). Nie benachbart: zwischen ihnen mindestens ein neutrales Element (v1: Text „Angehalten, weil keine Regel passt · Standard: fragen" `fg-1`). Beim Klick: Button füllt sich 120 ms mit Zustandsfarbe, Rail der Karte sweept 200 ms, dann `decide`. Während `decide` läuft: Buttons deaktiviert. Fehler ⇒ inline `HDiagnosticCard` unter der Leiste, kein Modal.

**Animationen.** Ankunft: `SlideTransition` 8 px von oben + `FadeTransition`, 180 ms, `Curves.easeOutCubic` (entspricht `cubic-bezier(0.2,0,0,1)` hinreichend; exakte Kurve als `Cubic(0.2, 0, 0, 1)`). Verlassen: `SizeTransition` (Höhe → 0) + Fade, 220 ms, `Cubic(0.4, 0, 1, 1)`; `Allow` gleitet 12 px nach rechts, `Block`/`TimedOut` 12 px nach links. Verwendung von `AnimatedList` oder `SliverAnimatedList` mit `visibleQueueFlowsProvider` und Diff über IDs.

**Shortcuts.** Auf `InterceptScreen`-Ebene: `AllowIntent` (Enter, `A`, Ctrl+F), `BlockIntent` (`B`, Ctrl+L), `NextFlowIntent` (`J`, ↓), `PrevFlowIntent` (`K`, ↑). Einzeltasten nur aktiv, wenn `!isTextInputFocused()`. Enter nur, wenn ein Flow ausgewählt und `held` ist.

**Domain-Pane (Platzhalter).** Host groß, Apex per `psl`-Dart-Port (kleine Funktion mit gebündelter PSL, HUM-031 ersetzt sie durch den Katalog), Karte gestrichelt „Nicht im Katalog", „Zuerst gesehen: jetzt", Buttons deaktiviert mit Tooltip „ab Sprint 2".

Leerzustand der Queue: zentriert, `fg-2`, Icon `inbox`, Text `intercept_empty_title` „Keine Anfrage wartet" und `intercept_empty_hint` „Der Agent arbeitet ohne Netz. Sobald er etwas anfragt, erscheint es hier." Kein Spinner.

### Schritte
1. Domain-`Flow`-Modell und Provider inklusive Reconnect und Resync; Unit-Tests gegen `FakeDaemonClient`.
2. `packages/ui`-Bausteine (`HCollapsible`, `HMethodBadge`, `HStateDot`, `HCountdownRing`) mit Galerie-Einträgen und Goldens.
3. Queue-Pane mit `AnimatedList`, Auswahl-Logik, Shortcuts J/K.
4. Request-Karte mit Sektionen und Maskierung.
5. Aktionsleiste mit Allow/Block, Animationen, Fehlerkarte.
6. Domain-Platzhalter; `ResizablePanel`-Verdrahtung mit persistenten Breiten (`SharedPreferences` unter Schlüssel `intercept.pane_ratios`).
7. Goldens und Integrationstest gegen Fake-Session bei 10×.

### Tests
- Unit `flows_apply_sequence`: Events `Received, Analyzed, Held` ⇒ ein Flow in `heldFlows`; `Decided(Allow)` ⇒ nicht mehr in `heldFlows`, aber 3 s in `visibleQueueFlows`.
- Unit `selection_never_stolen`: Auswahl auf Flow 1, Flow 2 kommt ⇒ Auswahl bleibt 1.
- Unit `selection_moves_on_leave`: Auswahl auf Flow 1 mit Deadline t1, Flow 2 (t2 > t1), Flow 3 (t3 > t2); Flow 1 entschieden ⇒ Auswahl auf Flow 2.
- Unit `resync_on_lagged`: `Lagged` ⇒ `listFlows` wird aufgerufen, Zustand gleicht Antwort.
- Unit `middle_ellipsis`: „/very/long/path/to/file.json" bei 16 ⇒ „/very/…file.json".
- Widget `enter_allows_selected`: Enter ⇒ `decide(id, Allow)` am Fake aufgerufen; ohne Auswahl ⇒ nichts.
- Widget `single_keys_ignored_in_textfield`: Fokus in einem Textfeld (Palette), `A` ⇒ kein `decide`.
- Widget `headers_masked_by_default`: `Authorization: Bearer x` wird als `••••` gerendert, Toggle zeigt Wert.
- Widget `timeout_marks_card`: `TimedOut`-Event ⇒ Karte zeigt `intercept_timed_out_banner`, Buttons deaktiviert.
- Golden `queue_row_idle`, `queue_row_selected`, `queue_row_hover`, `request_card_basic`, `intercept_empty`, `intercept_three_held` (dark und light).
- Integration (xvfb): Fake bei 10× spielt 30 Flows; keine Frame-Drops > 100 ms (`WidgetTester.binding` Frame-Timing), Queue zählt am Ende 0 offene, `decide` wurde 30× aufgerufen durch scripted Enter.

### Akzeptanzkriterien
- [ ] Gegen echten Daemon: `humanitl sandbox run -- curl https://example.com` erscheint in < 300 ms in der Queue; Enter ⇒ curl liefert HTML; `B` ⇒ curl zeigt 403-Text. Daemon-Hälfte belegt (`m1_sealed_box.sh:189,214`); kein Test fährt den Bildschirm gegen einen laufenden Daemon, die 300 ms misst niemand; die Oberflächen-Hälfte des Laufs ist HUM-097.
- [x] Countdown stimmt auf ± 1 s mit dem Daemon-Timeout überein; Ablauf ⇒ Banner, keine Exception. (Frist reist als absoluter Timestamp, ein `nowProvider` mit 250 ms; nie gegen eine lebende Daemon-Frist verglichen)
- [x] Alle Goldens grün in CI-Modus; `flutter analyze` sauber; keine Strings außerhalb ARB.
- [ ] Panes lassen sich ziehen, Breiten überleben Neustart. Ziehen ja (`intercept_screen_test.dart:379`); Breiten überleben keinen Neustart: `shared_preferences` ist keine Abhängigkeit, `paneRatiosSettingsKey` wird nie gelesen oder geschrieben (`pane_layout.dart:7-22`), und der Verzicht steht nur im Quellkommentar.
- [ ] Bei 200 gehaltenen Flows (Fake) bleibt Scrollen flüssig (`ListView.builder`/`AnimatedList` mit `itemExtent`). Faulheit belegt (200 Flows, 5 bis 60 gebaute Zeilen); die Queue ist eine `AnimatedList` ohne `itemExtent`, und der spezifizierte xvfb-Lauf mit Frame-Budget existiert nicht.

### Fallstricke
- `AnimatedList` verlangt Index-Buchführung; bei Diff über IDs eine kleine `ListDiff`-Hilfsfunktion schreiben und testen, sonst Exceptions „index out of range" bei schnellen Änderungen.
- Ein `Timer` pro Zeile skaliert nicht; deshalb der zentrale `nowProvider`.
- `ResizablePane` in shadcn_flutter 0.0.54 hat offene Bugs (#427/#428); wenn Ziehen hakt, `multi_split_view` hinter dem `HResizable`-Wrapper einsetzen, ohne Screen-Code zu ändern.
- Enter in der Command Palette darf keinen Allow auslösen: Palette setzt `isTextInputFocused()` wahr; zusätzlich `AllowIntent` nur bei sichtbarer Intercept-Sektion.
- `SelectableText` mit sehr langen Zeilen (Body ohne Umbruch) ist langsam; Body-Raw in v1 auf 64 KB begrenzen und `softWrap: true`.
- Deadline vom Daemon kommt als absoluter Zeitstempel; Uhrenversatz zwischen Daemon und App ist null (gleiche Maschine), aber `Instant`-basierte Deadlines des Daemons müssen im Proto als `Timestamp` gesendet werden (HUM-018 `convert.rs`: `Instant` ⇒ `SystemTime` beim Erzeugen des Events, nicht beim Senden).

### Referenzen
BACKLOG.md 5 (Layout, Interaktion, Motion, Tokens), Flutter/UX-Lead-Review 2 und 4, Visual-Designer 4 und 5; CONVENTIONS 3.9; two_dimensional_scrollables https://pub.dev/packages/two_dimensional_scrollables ; alchemist https://pub.dev/packages/alchemist

---

## HUM-021 · Demo-Skript M1
Sprint: 1 · Größe: S · Abhängigkeiten: alle Sprint-1-Issues · Blockiert: Sprint 2

### Kontext
Merge-Bedingung aus BACKLOG.md 8: Jeder Sprint endet mit einem grünen Demo-Skript in CI. M1 beweist: Sandbox dicht, Request gehalten, Entscheidung wirkt.

### Ziel
`tests/e2e/m1_sealed_box.sh` startet den Daemon, prüft die Isolation, feuert einen `curl` aus der Sandbox, sieht ihn als `Held` im gRPC-Stream, blockt ihn per CLI-Test-Client, prüft die 403-Antwort, erlaubt einen zweiten Request und prüft den Inhalt. Läuft lokal und im CI-Job `e2e` (ohne UI; der UI-e2e-Job kommt mit HUM-036).

### Nicht-Ziel
UI-Automation. Ollama.

### Betroffene Pfade
- `tests/e2e/m1_sealed_box.sh` (neu)
- `tests/e2e/lib.sh` (neu): Helfer `start_daemon`, `stop_daemon`, `wait_for_socket`, `grpc_decide`
- `daemon/bin/humanitl/src/cmd/flows.rs`: `humanitl flows decide ID allow|block` als Test-Hilfskommando (bleibt im Produkt; die CLI-Entscheidung ist der `--ask terminal`-Vorläufer)
- `.github/workflows/ci.yml`: Job `e2e`

### Spezifikation

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "$0")/lib.sh"

export XDG_RUNTIME_DIR=$(mktemp -d) XDG_DATA_HOME=$(mktemp -d) XDG_CONFIG_HOME=$(mktemp -d)
start_daemon --hold-timeout-secs 5            # Hintergrund, wartet auf daemon.sock
trap stop_daemon EXIT

# 1. Isolation
humanitl sandbox check                        # Exit 0, drei ✓
humanitl sandbox run --profile test -- /tests/escape/esc-1.sh
humanitl sandbox run --profile test -- /tests/escape/esc-2.sh

# 2. Ohne Proxy-Env kein Weg
out=$(humanitl sandbox run -- sh -c 'env -i /usr/bin/curl -sS --max-time 3 http://example.com/ 2>&1 || true')
grep -q -E 'Could not resolve host|Network is unreachable|Couldn.t connect' <<<"$out"

# 3. Gehalten und geblockt
work=$(mktemp -d)
( humanitl sandbox run --work "$work" -- curl -sS -o "$work/out.txt" -w '%{http_code}' https://example.com/ > "$work/code.txt" ) &
flow=$(wait_for_held 10)                      # pollt `humanitl flows list --json state:held`, liefert erste ID
[[ "$(humanitl flows show "$flow" --json | jq -r .host)" == "example.com" ]]
humanitl flows decide "$flow" block
wait
[[ "$(cat "$work/code.txt")" == "403" ]]
grep -q '^reason: user' "$work/out.txt"

# 4. Gehalten und erlaubt (gegen lokalen Fake-Upstream, damit CI offline bleibt)
start_fake_upstream                            # Port in $FAKE_HTTP, siehe HUM-017 Support-Binary `humanitl-fake-upstream`
( humanitl sandbox run --work "$work" -- curl -sS -o "$work/out2.txt" -w '%{http_code}' "http://host.humanitl.internal:$FAKE_HTTP/echo" > "$work/code2.txt" ) &
flow=$(wait_for_held 10)
humanitl flows decide "$flow" allow
wait
[[ "$(cat "$work/code2.txt")" == "200" ]]
jq -e '.path == "/echo"' "$work/out2.txt"

# 5. Timeout
( humanitl sandbox run --work "$work" -- curl -sS -o /dev/null -w '%{http_code}' "http://host.humanitl.internal:$FAKE_HTTP/echo" > "$work/code3.txt" ) &
wait
[[ "$(cat "$work/code3.txt")" == "403" ]]
echo "M1 demo: OK"
```

`host.humanitl.internal` ist ein Name, den der Proxy-Upstream-Resolver im Testmodus (`--test-resolve host.humanitl.internal=127.0.0.1`, Daemon-Flag nur mit Cargo-Feature `test-hooks`) auf Loopback des Hosts auflöst. Das zeigt zugleich, dass die Sandbox selbst keinen Namen auflöst und der Daemon es nach Freigabe tut.

Schritt 2 ist die Kontrollprobe für Garantie 1: Auch ein Prozess, der `HTTP_PROXY` ignoriert (`env -i`), kommt nicht raus.

### Schritte
1. `lib.sh` mit robustem Warten (Polling, Timeouts, klare Fehlermeldungen).
2. `humanitl flows decide`.
3. `humanitl-fake-upstream`-Binary aus HUM-017-Support herauslösen (`daemon/bin/humanitl-fake-upstream`, Cargo-Feature `dev-tools`).
4. Skript, lokal grün.
5. CI-Job `e2e`: baut Daemon, Shim (musl), CLI, Fake-Upstream, installiert `bubblewrap curl jq`, führt Skript aus, lädt Daemon-Log als Artefakt hoch.

### Tests
Das Skript ist der Test. Zusätzlich `tests/e2e/README.md` mit Ablauf und Erwartungen.

### Akzeptanzkriterien
- [x] Skript grün lokal (Debian) und im CI (`ubuntu-latest`) in < 90 s. (24,6 s mit `E2E_SKIP_BUILD=1`, 41 Behauptungen; der CI-Job läuft auf `ubuntu-24.04`)
- [x] Bei jedem `[[ … ]]`-Fehlschlag steht der Schritt im Log (`set -x` in CI).
- [ ] Daemon-Log enthält für Schritt 3 keine DNS-Auflösung von `example.com` (Prüfung: `--test-hooks` zählt Resolver-Aufrufe, Skript prüft `humanitl daemon status --json | jq .test.resolves == 0` vor `decide` und `== 1` nach dem Allow in Schritt 4). Es gibt weder `--test-hooks` noch ein Feld `test` in `daemon status --json` (sechs Felder, `daemon.rs:40-47`); das Skript ersetzt das Kriterium durch Schritt 2 (in der Sandbox löst nichts auf), was über den Daemon vor der Entscheidung nichts sagt (HUM-115).

### Fallstricke
- `ubuntu-latest` und AppArmor: der Job setzt `sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0` vorab; wenn das fehlschlägt, muss `SANDBOX_003` sauber erscheinen und der Job rot sein, nicht hängen.
- `wait` in Bash liefert den Exit-Code des letzten Hintergrundjobs; Subshells so bauen, dass `set -e` nicht das Skript vorzeitig beendet (`|| true` an den richtigen Stellen).
- `curl` in der Sandbox ist das Host-`/usr/bin/curl` via ro-Bind; auf dem CI-Runner vorhanden.
- `/tests/escape/*.sh` müssen in der Sandbox sichtbar sein: Profil `test.toml` bindet das Repo-Verzeichnis `tests/escape` read-only nach `/tests/escape`.

### Referenzen
BACKLOG.md 7 (M1), 8 (Merge-Bedingung), 4.5; CONVENTIONS 3.11.

---

## HUM-108 · `PROXY_007` wird nie ausgegeben
Sprint: 1 · Größe: S · Abhängigkeiten: HUM-017, HUM-045 · Blockiert: keine

### Kontext
Das Register kennt `PROXY_007` („HTTP/2 nicht verfügbar", `codes.rs:297`), `docs/SECURITY.md:438-441` und `:602-604` versprechen, dass gRPC über TLS „sichtbar" mit diesem Befund scheitert, und CONVENTIONS 4.10 hält Zeile 13 der Matrix als „erwartet: fehlschlägt mit `PROXY_007 h2 not available`" fest. Kein Pfad erzeugt den Befund: `grep -rn PROXY_007 daemon/crates daemon/bin --include='*.rs'` trifft nur Doc-Kommentare, das Register und eine Assertion-Nachricht in `conformance.rs:772`. `tls_observe::classify` deutet nur `rustls::Error::AlertReceived` und einige `io::ErrorKind`; der ALPN-Fehlschlag `rustls::Error::NoApplicationProtocol` fällt auf `None`, es bleibt eine `tracing::debug!`-Zeile, und der Agent sieht einen nackten TLS-Alert `no application protocol`. Das verstößt gegen die Regel aus `CLAUDE.md`: jeder Fehlerpfad liefert ein `Diagnostic` mit `why`. Derselbe Lauf hat gezeigt, dass `PROXY_001` („Body über Cap") ebenso nie gebaut wird; das entscheidet HUM-057 mit `LIMIT_001`, nicht dieses Issue.

### Ziel
Ein Client, der in ALPN nur `h2` anbietet (grpcurl über TLS, Zeile 13 der Matrix), bekommt weiter den TLS-Alert, und der Daemon veröffentlicht dazu genau einen Befund `PROXY_007` (Warning) im Ereignisstrom, mit Host, dem Satz, warum es scheitert, und einem Verweis auf die Dokumentation; entstört je Host wie `TLS_002`. Die Karte aus HUM-106 kann ihn zeigen. Dazu ein Lint, der den nächsten toten Registereintrag verhindert.

### Nicht-Ziel
HTTP/2 selbst (M6, `experimental.h2_upstream`). `PROXY_001` (HUM-057 entscheidet, ob `LIMIT_001` ihn ersetzt oder er gebaut wird). Die Karte in der Oberfläche (HUM-106 zeigt heute `TLS_001..003`; dort kommt `PROXY_007` in den Filter, eine Zeile).

### Betroffene Pfade
- `daemon/crates/proxy/src/tls_observe.rs` (`TlsFailure::NoApplicationProtocol`, `classify`, `diagnostic_for`)
- `daemon/crates/proxy/src/handler.rs` (Aufruf bei `handler.rs:398-424`, Entstörung über `HandshakeWatch`)
- `daemon/crates/proxy/tests/tls_observe.rs`, `daemon/crates/proxy/tests/conformance.rs` (Zeile 13)
- `scripts/ci/lint-dead-codes.sh` (neu), `scripts/ci/dead-codes-allow.txt` (neu), `Makefile`, `.github/workflows/ci.yml` (Job `rust-check`)
- `docs/DIAGNOSTICS.md` (Generatorlauf, Titel bleibt)

### Spezifikation
```rust
pub enum TlsFailure {
    // ...
    /// Client und Proxy haben kein gemeinsames ALPN-Protokoll: der Client bot
    /// nur `h2`, der Proxy spricht in diesem Milestone `http/1.1`.
    NoApplicationProtocol,
}
// classify: rustls::Error::NoApplicationProtocol => Some(TlsFailure::NoApplicationProtocol)
// as_str: "no_application_protocol"

pub fn diagnostic_for(failure: &TlsFailure, host: &HostName, hint: ToolHint, since_last: u32) -> Diagnostic
// NoApplicationProtocol => h2_not_available(host, since_last)

fn h2_not_available(host: &HostName, since_last: u32) -> Diagnostic {
    Diagnostic::builder(PROXY_007, Severity::Warning)
        .why(format!("{host}: the client offered only h2 in ALPN; the proxy speaks HTTP/1.1 in this milestone, so gRPC over TLS and other h2-only clients cannot connect through it"))
        .fix(FixAction::OpenUrl(docs_url("#proxy_007")))
        .build()
}
```

Entstörung: dieselbe `HandshakeWatch` und dasselbe 60-s-Fenster je Host wie `TLS_002`; der Zähler unterdrückter Versuche steht ab zwei im Text. Ereignis: `FlowEvent::Diagnostic` ohne Flow (es gibt keine Anfrage; der Handschlag scheitert vor der ersten Zeile). Die `tracing::debug!`-Zeile bleibt.

Lint `scripts/ci/lint-dead-codes.sh`: liest jeden Code aus dem `registry!`-Block in `codes.rs`, sucht eine Konstruktion (`codes::AREA_NNN` oder der nackte Bezeichner) außerhalb von `codes.rs`, Doc-Kommentaren und `#[cfg(test)]`-Blöcken; ein Code ohne Fundstelle endet 1 mit Nennung des Codes, außer er steht mit Begründung in `scripts/ci/dead-codes-allow.txt` (eine Zeile je Code: `PROXY_001 HUM-057`, `TERM_001 HUM-042`, `AUDIT_001 HUM-050`, `UI_002 app/lib/core/domain/diagnostic_codes.dart`). Ein Eintrag der Allow-Liste, der nicht mehr tot ist, endet ebenfalls 1, damit die Liste schrumpft. Teil von `make check` und des Jobs `rust-check`.

### Schritte
1. `TlsFailure::NoApplicationProtocol`, `classify`, `as_str`. Prüfen: Unit-Test mit `rustls::Error::NoApplicationProtocol` liefert die Variante.
2. `diagnostic_for` → `PROXY_007`. Prüfen: Test assertet Code, Severity Warning, `why` enthält Host und `h2`.
3. Anschluss an `HandshakeWatch` in `handler.rs`. Prüfen: zwei Handschläge in 60 s ergeben ein Ereignis mit Zähler 2.
4. Zeile 13 der Matrix erweitern: nach dem grpcurl-Fehlschlag `events.wait_for("diagnostic")`, Code `PROXY_007`.
5. Lint-Skript, Allow-Liste, Makefile, CI. Prüfen: Selbsttest mit einem Fixture-Register, das einen unbenutzten Code trägt, endet 1.
6. `docs/SECURITY.md` gegenlesen: `:438` und `:602` sind mit diesem Issue wahr; keine Textänderung nötig, außer HUM-111 hat sie vorher umformuliert.

### Tests
- `no_application_protocol_is_classified` (Unit, `tls_observe.rs`).
- `alpn_refusal_yields_proxy_007` (Unit): `diagnostic_for(&TlsFailure::NoApplicationProtocol, ...)`.
- `alpn_refusal_reaches_the_event_stream` (Integration, `tests/tls_observe.rs`): ein rustls-Client mit `alpn_protocols = [b"h2"]` gegen `tls::accept` des Proxys; danach genau ein `Diagnostic`-Ereignis mit `PROXY_007`.
- `conf_13_grpcurl_over_tls_fails_visibly` (erweitert): Alert wie bisher plus der Befund im Strom.
- `lint_dead_codes_self_test`: Fixture mit unbenutztem `PROXY_099` → Exit 1; Allow-Listen-Eintrag mit lebendem Code → Exit 1.

### Akzeptanzkriterien
- [ ] `grep -rn 'codes::PROXY_007' daemon/crates/proxy/src` trifft eine Konstruktion in `tls_observe.rs`, nicht nur Doc-Kommentare.
- [ ] `cargo test -p humanitl-proxy --test conformance conf_13` grün, und der Test assertet den Befund `PROXY_007` im Ereignisstrom.
- [ ] Gegen einen laufenden Daemon: `grpcurl -cacert <ca.crt> <host>:443 list` über den Proxy scheitert, und das Daemon-Log trägt eine `warn`-Zeile mit `code=PROXY_007` und dem Host (Blick).
- [ ] `scripts/ci/lint-dead-codes.sh` endet 0 auf dem Baum; nach Einfügen eines Codes `PROXY_099` ins Register ohne Verwendung endet es 1 und nennt den Code; `scripts/ci/dead-codes-allow.txt` trägt genau `PROXY_001`, `TERM_001`, `AUDIT_001` und `UI_002`, jeder mit Issue oder Datei.
- [ ] `docs/DIAGNOSTICS.md` nach dem Generatorlauf unverändert (`docs_in_sync` grün); `make check` grün.

### Fallstricke
- rustls liefert `NoApplicationProtocol` serverseitig nur, wenn `alpn_protocols` nicht leer ist und keine Schnittmenge besteht (`ca.rs:601` setzt `[http/1.1]`); ein Client ohne ALPN bekommt `http/1.1` ohne Fehler. Nicht verwechseln.
- Keinen Flow für den gescheiterten Handschlag anlegen; es gibt keine Anfrage.
- `host` stammt aus dem CONNECT-Ziel und ist vom Agenten gewählt; im `why` wie bei `TLS_001` über `host.display()` und ohne Steuerzeichen.
- Die Allow-Liste darf kein Friedhof werden: jeder Eintrag braucht eine Issue-Nummer oder die Datei, die den Code baut; wer einen Eintrag ohne Begründung sieht, löscht ihn oder den Code.

### Referenzen
`backlog/sprint-1.md` HUM-017 (Zeile 13), `backlog/sprint-3.md` HUM-045 (`tls_observe.rs`, `HandshakeWatch`); CONVENTIONS 4.6, 4.10; `docs/SECURITY.md` 5, 10.5; `daemon/crates/core-types/src/diagnostics/codes.rs:284, 297`; `CLAUDE.md` „Was der Compiler nicht prüft"; rustls 0.23 `Error::NoApplicationProtocol`.

---

## HUM-109 · `HoldQueue::extend` ist von nichts erreichbar
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-016, HUM-018, HUM-064 · Blockiert: keine

### Kontext
`HoldQueue::extend` (`daemon/crates/proxy/src/hold.rs:414`) schiebt die Frist eines gehaltenen Flows nach hinten und ist mit drei Tests belegt (`tests/hold.rs:233, 644, 795`). Von außen erreicht es niemand: kein Proto-Feld (`grep -n extend proto/humanitl/v1/humanitl.proto` leer), kein RPC in `daemon/crates/ipc/src/server.rs`, kein Unterkommando, keine Dart-Methode. Die Spezifikation von HUM-016 nennt den Zweck in einer Zeile („‚Timer pausieren' im UI = extend um 24 h, audit-geloggt (HUM-050)"), `docs/UX.md:362` verspricht neben der Vorwarnung „genau eine Verlängerung, sobald der Daemon Verlängern kennt". Totes Verhalten mit grünen Tests ist das Muster, das dieses Repository schon dreimal getäuscht hat (`CLAUDE.md`, 2026-09-03). ADR-018: jede Fähigkeit ist zuerst ein RPC; ADR-004: Ereignisse werden aus Übergängen abgeleitet.

**Vorher zu entscheiden.** Verdrahten (diese Spezifikation) oder entfernen (Alternative unten). Empfehlung: verdrahten. Eine Frist von fünf Minuten ohne Verlängerung macht aus jeder Kaffeepause einen Timeout-Block, und `docs/UX.md` hat die Verlängerung schon zugesagt.

### Ziel
`humanitl flows extend <id> [--by SECS]` und der RPC `Extend` verschieben die Frist eines gehaltenen Flows; jeder Abonnent sieht die neue Frist als Ereignis; ein nicht mehr gehaltener Flow antwortet `IPC_003`; die Zustandsmaschine kennt den Übergang `Held → Held`; HUM-050 kann ihn später ins Audit schreiben.

### Nicht-Ziel
Der Knopf in der Aktionsleiste (Oberflächen-Hälfte; sie bekommt nach dem Muster HUM-105/HUM-106 eine eigene Nummer, sobald der RPC steht, und setzt `docs/UX.md:362` um: genau eine Verlängerung neben der Vorwarnung). Der Audit-Eintrag (HUM-050). Eine Obergrenze der Gesamthaltezeit (die Speichergrenze für gehaltene Bodies ist HUM-057).

### Betroffene Pfade
- `proto/humanitl/v1/humanitl.proto` (`rpc Extend`, `ExtendRequest`, `ExtendResponse`, `FlowEvent.extended = 17`), `proto/descriptor.binpb`, `proto/generated.sha256`
- `daemon/crates/core-types/src/flow.rs` (`TransitionInput::Extend`, `FlowEvent::Extended`, Übergang `Held → Held`), `tests/flow_state_table.rs`
- `daemon/crates/proxy/src/hold.rs` (bleibt), `src/handler.rs` oder `src/registry.rs` (Anwendung über `Flow::apply`)
- `daemon/crates/ipc/src/server.rs`, `src/convert.rs`, `src/validate.rs`, `src/fake/state.rs`, `src/lib.rs` (`PROTO_MINOR`)
- `daemon/bin/humanitl/src/cli.rs`, `src/cmd/flows.rs`
- `app/lib/core/ipc/daemon_client.dart`, `grpc_daemon_client.dart`, `fake_daemon_client.dart`, `app/lib/core/ipc/convert.dart`, `app/lib/core/domain/flow_event.dart`, `app/lib/core/domain/flow_reducer.dart` (HUM-098), `app/lib/core/ipc/proto_version.dart`

### Spezifikation
```proto
rpc Extend(ExtendRequest) returns (ExtendResponse);

message ExtendRequest {
  string flow_id = 1;
  // Sekunden, um die die Frist nach hinten rückt. 0 bedeutet die Vorgabe 86400.
  uint32 by_secs = 2;
}
message ExtendResponse { google.protobuf.Timestamp deadline = 1; }

// in FlowEvent.event:
//   Extended extended = 17;
message Extended { string flow_id = 1; google.protobuf.Timestamp deadline = 2; }
```

```rust
// humanitl-core
pub enum TransitionInput { /* ... */ Extend { deadline: Instant } }
pub enum FlowEvent { /* ... */ Extended { flow_id: FlowId, at: Instant, deadline: Instant } }
// Erlaubt genau: Held + Extend -> Held. Jeder andere Zustand: InvalidTransition.
```

Reihenfolge im Proxy: `HoldQueue::extend(id, by)` liefert die neue Frist; danach `Flow::apply(Extend { deadline })` am registrierten Flow (einziger Mutationspunkt, wie im Fake `state.rs:330`); das Ereignis geht in den Strom. Scheitert `extend` mit `NotHeld`, gibt es kein Ereignis und der RPC antwortet `IPC_003`. Eine unlesbare Id ist `IPC_004`. `by_secs` über 7 Tagen ist `IPC_004` mit dem Wert im `why`.

IPC: `PROTO_MINOR` 3 → 4, `app/lib/core/ipc/proto_version.dart` dieselbe Zahl (`docs/PROTOCOL.md` 5). Der Rust-Fake (`fake/state.rs`) setzt den RPC über denselben `apply` um, damit `humanitld --fake` ihn spielt.

CLI (`docs/PROTOCOL.md` 4.9, ein RPC bringt sein Unterkommando mit): `humanitl flows extend <id> [--by SECS]`, Textausgabe `deadline <RFC 3339>`, `--json` `{"flow_id","deadline"}`; Exit 1 mit dem Befund des Daemons bei `IPC_003`/`IPC_004`.

Dart: `DaemonClient.extendHold(FlowId id, Duration by)` in allen drei Clients; `FlowEvent.extended(flowId, deadline)`; der Reduzierer (`applyFlowEvent`, HUM-098) setzt `deadline`. Der Countdown-Ring folgt damit ohne weitere Änderung, weil er aus `flow.deadline` rechnet.

**Alternative (entfernen), falls so entschieden:** `HoldQueue::extend` und seine drei Tests löschen, die Zeile in `backlog/sprint-1.md` HUM-016 (`:629`) bekommt den Vermerk, `docs/UX.md:362` verliert den Halbsatz über die Verlängerung, und `grep -n 'fn extend' daemon/crates/proxy/src/hold.rs` ist leer. Dann Größe S.

### Schritte
1. Core: `TransitionInput::Extend`, `FlowEvent::Extended`, Tabellenzeile `Held + Extend → Held`, Zeugentest und Zähler in `flow_state_table.rs` nachziehen. Prüfen: `cargo test -p humanitl-core` grün, `forbidden_transitions_are_errors` rechnet mit 13 Eingaben.
2. Proto, `descriptor.binpb`, `generated.sha256`, `PROTO_MINOR`, `proto_version.dart`. Prüfen: `make proto && git diff --exit-code proto/` leer.
3. Proxy: Anwendung über `Flow::apply`, Ereignis im Strom. Prüfen: `extend_publishes_extended_event`.
4. IPC: `Extend` im Server und im Fake, Validierung. Prüfen: `extend_rpc_moves_the_deadline_and_streams_it`, `extend_of_a_decided_flow_is_ipc_003`.
5. CLI `flows extend`. Prüfen: `flows_extend_prints_the_new_deadline` gegen `humanitld --fake`.
6. Dart-Clients und Reduzierer. Prüfen: `flutter test test/core` grün, `apply_extended_moves_the_deadline`.

### Tests
- `extend_only_from_held` (core): jeder andere Zustand ergibt `InvalidTransition`.
- `extend_moves_deadline` (bleibt), `extend_publishes_extended_event` (proxy).
- `extend_rpc_moves_the_deadline_and_streams_it`, `extend_of_a_decided_flow_is_ipc_003`, `extend_over_seven_days_is_ipc_004` (ipc, gegen einen Unix-Socket wie `rules_rpc.rs`).
- `flows_extend_prints_the_new_deadline`, `flows_extend_unknown_flow_exit_1` (cli).
- `apply_extended_moves_the_deadline` (dart, `flow_reducer_test.dart`), `fake_extend_hold_emits_extended` (dart fake).

### Akzeptanzkriterien
- [ ] `grep -n 'rpc Extend' proto/humanitl/v1/humanitl.proto` trifft; `PROTO_MINOR` ist 4 in `daemon/crates/ipc/src/lib.rs` und `app/lib/core/ipc/proto_version.dart`; `proto/descriptor.binpb` liegt im selben Commit.
- [ ] Gegen einen laufenden Daemon: ein `curl` aus der Sandbox wird gehalten; `humanitl --json flows extend <id> --by 60` liefert Exit 0, und `humanitl --json flows show <id> | jq -r .deadline` liegt 60 s hinter dem Wert von vor dem Aufruf (Blick).
- [ ] `humanitl flows extend <id>` für einen entschiedenen Flow liefert Exit 1 mit `IPC_003`.
- [ ] `cargo test -p humanitl-core` grün, `forbidden_transitions_are_errors` rechnet mit der neuen Eingabe; `cargo test -p humanitl-proxy --test hold` und `cargo test -p humanitl-ipc` grün.
- [ ] `grep -n 'extendHold' app/lib/core/ipc/daemon_client.dart app/lib/core/ipc/grpc_daemon_client.dart app/lib/core/ipc/fake_daemon_client.dart` trifft in allen drei Dateien; `flutter test test/core` grün.
- [ ] `make check` grün, `tools/verify-commit.sh` grün.
- [ ] Bei Entscheidung „entfernen" statt der Punkte oben: `grep -rn 'fn extend\|extend_moves_deadline' daemon/crates/proxy` leer, die Spezifikationszeile in HUM-016 trägt den Vermerk, `docs/UX.md:362` ist angepasst.

### Fallstricke
- `HoldQueue::extend` ändert nur `pending.deadline`; die Halte-Task muss die verschobene Frist auch abwarten. `tests/hold.rs:233` belegt das (Entscheidung nach 500 ms kommt bei `timeout = 200 ms` noch durch); den Test nicht abschwächen.
- Keinen zweiten Mutationspunkt einführen: `grep 'state =' daemon/crates/ipc/src/fake/` bleibt ohne Zustandszuweisung (HUM-005, Kriterium 3).
- Die Frist reist als absoluter `Timestamp` aus einem `Instant` (`convert.rs:699` `wall_clock`); denselben Umrechner benutzen, sonst driften Ring und Daemon.
- Der Daemon erlaubt wiederholtes Verlängern; die Oberfläche bietet nach `docs/UX.md:362` genau eine. Das ist eine Entscheidung der Oberfläche, nicht des Daemons, und steht so in CONVENTIONS 4.x.
- `Held.queue_bytes`/`queue_count` ändern sich durch `extend` nicht; das Ereignis trägt sie nicht.

### Referenzen
`backlog/sprint-1.md` HUM-016 (Spezifikation `:629`, Test `extend_moves_deadline`), HUM-018; BACKLOG.md ADR-004, ADR-018; `docs/PROTOCOL.md` 4.9, 5; `docs/UX.md:362`; `backlog/sprint-4.md` HUM-050 (Audit), `backlog/sprint-5.md` HUM-098 (`applyFlowEvent`); `daemon/crates/proxy/src/hold.rs:404-421`.

---

## HUM-110 · WebSocket-Upgrade kommt nie zustande
Sprint: 1 · Größe: L · Abhängigkeiten: HUM-015, HUM-017, HUM-026 · Blockiert: HUM-058 (Zeile 11 der Zustandsmatrix)

### Kontext
Der Proxy entfernt `connection` und `upgrade` als Kopfzeilen von Verbindungsrang (`daemon/crates/proxy/src/upstream.rs:48-58`). Das Ziel sieht die Anfrage deshalb ohne `Upgrade` und antwortet 426; `conformance.rs:629` assertet genau das und `:662` sagt es aus: „M1 has no upgrade passthrough". Versprochen ist das Gegenteil an vier Stellen: BACKLOG.md 2 (ADR-001, Protokoll-Ziel M1 „WebSocket-Passthrough"), ADR-007 („danach werden Frames aufgezeichnet, nicht angehalten. Das wird im UI ausgesprochen"), `docs/SECURITY.md:471-472` und `:605-606` („Nach einem freigegebenen Upgrade fließen Frames und werden aufgezeichnet") und `backlog/sprint-5.md` HUM-058 Zeile 11 mit dem Oberflächentext „Nachrichten danach werden aufgezeichnet, nicht angehalten". HUM-015 hat den Passthrough als Nicht-Ziel an HUM-026 verwiesen, HUM-026 hat ihn nicht gebaut; die Regeln kennen `upgrade: websocket`, der Recorder die Spalte `upgrade`, beides ohne Anfrage, die je durchkäme. CONVENTIONS 4.10 sanktioniert nur den dokumentierten Fehlschlag der gRPC-Zeile, nicht diesen. Festgehalten ist der Zustand in einem Kommentar in einem Test.

### Ziel
Nach `Decided(Allow)` auf einem Flow mit `Upgrade::WebSocket` reicht der Proxy den Handschlag mit `Connection: Upgrade` und `Upgrade: websocket` an das Ziel, gibt dessen 101 an den Client zurück und kopiert danach Bytes in beide Richtungen, bis eine Seite schließt. Der Flow durchläuft `Forwarded → Responded{101} → Recorded`; die Bytes vom Ziel zum Client werden wie eine gestreamte Antwort aufgezeichnet, die Bytes vom Client zum Ziel gezählt. Zeile 11 der Matrix assertet 101 und ein funktionierendes Echo. Block und Timeout vor dem Upgrade bleiben, wie sie sind (403, 504). Regeln mit `upgrade: websocket` entscheiden wie heute; `wss://` läuft nach dem CONNECT denselben Weg.

### Nicht-Ziel
Frames halten (BACKLOG.md 9.8, `experimental.ws_hold` bleibt `pending`, HUM-101). Frames zerlegen und je Opcode aufzeichnen (dieses Issue zeichnet die Rohbytes Ziel → Client auf; die Frame-Zerlegung entscheidet der Eigentümer mit 9.8). Findings-Scan auf Frames. Andere Upgrades (`h2c`, `TLS/1.0` über `Upgrade`): sie bleiben gestrichen und enden wie heute.

### Betroffene Pfade
- `daemon/crates/proxy/src/upstream.rs` (Ausnahme von `HOP_BY_HOP` für das freigegebene WebSocket-Upgrade)
- `daemon/crates/proxy/src/handler.rs` (101-Pfad: `hyper::upgrade::on` auf beiden Seiten, `tokio::io::copy_bidirectional`)
- `daemon/crates/proxy/src/body.rs` (Spiegel der Ziel-Bytes als `ResponseChunk`)
- `daemon/crates/proxy/tests/conformance.rs` (Zeile 11), `tests/proxy.rs` (neue Fälle), `tests/support/mod.rs` (Fake-Upstream mit Echo-Endpunkt `/ws`, heute antwortet er 426)
- `daemon/crates/recorder/src/writer.rs` (Status 101, `upgrade = 'websocket'`, Antwort-Bytes bis `recorder.max_body_bytes`)
- `proto/humanitl/v1/humanitl.proto` (`FlowDetail.upgrade_client_bytes = <nächste Nummer>`), `PROTO_MINOR`
- `docs/SECURITY.md` 6 und 10.3, `backlog/CONVENTIONS.md` 4.10 (Zeile 11)

### Spezifikation
Kopfzeilen: `HOP_BY_HOP` bleibt die Regel. Für einen Flow mit `RequestKey.upgrade == Some(WebSocket)` und `Decision::Allow` (oder `AllowEdited`) behält der Proxy genau `Connection: Upgrade`, `Upgrade: websocket` und die `Sec-WebSocket-Key`/`-Version`/`-Protocol`-Kopfzeilen (RFC 9110 §7.8: ein Proxy, der das Upgrade unterstützt, leitet es weiter). `Sec-WebSocket-Extensions` wird entfernt, damit aufgezeichnete Bytes nicht `permessage-deflate`-komprimiert sind; das Ziel darf dann keine Extension aushandeln, und eine 101-Antwort mit `Sec-WebSocket-Extensions` ist ein Protokollfehler (502, `reason: upstream`). Andere `Connection`-Token werden weiter gestrichen.

Antwortpfad: Antwortet das Ziel 101 mit `Upgrade: websocket`, wird `hyper::upgrade::on` auf der Antwort (Ziel) und auf der Anfrage (Client, `with_upgrades()` steht in `handler.rs:1210`) abgewartet, dann `tokio::io::copy_bidirectional`. Bytes Ziel → Client gehen zusätzlich durch den Spiegel aus `body.rs` (Ereignisse `ResponseChunk`, Vorschau-Cap, `recorder.max_body_bytes`); Bytes Client → Ziel werden gezählt und in `FlowEvent::Recorded` beziehungsweise `FlowDetail.upgrade_client_bytes` gemeldet. `Responded { status: 101 }` beim Eintreffen der 101, `Recorded`, wenn beide Richtungen geschlossen sind. Ein 101 ohne `Upgrade: websocket` oder auf eine Anfrage ohne Upgrade ist ein Protokollfehler des Ziels: 502, `reason: upstream`, `Failed`. Jede andere Antwort (200, 404, 426) läuft den normalen Antwortpfad.

Zeitgrenzen: die Leerlaufgrenze aus HUM-101 gilt nicht für eine aufgewertete Verbindung; die Verbindung lebt, bis eine Seite schließt oder die Sitzung endet.

Recorder: `flows.status = 101`, `flows.upgrade = 'websocket'` (steht schon), Antwort-Body die aufgezeichneten Bytes bis zur Grenze mit `truncated`, `response_size` die Gesamtzahl.

Zeile 11 der Matrix neu: curl-Handschlag mit `--http1.1 -i` liefert 101; websocat über den `ws-c:`-Overlay sendet eine Nachricht und bekommt sie vom Echo-Endpunkt zurück; danach `held` 1, `recorded` 1, `status 101`.

### Schritte
1. Fake-Upstream in `tests/support`: `/ws` als echter Echo-Endpunkt (`tokio-tungstenite` als Dev-Abhängigkeit oder ein handgeschriebener Minimal-Server, der genau Echo kann). Prüfen: direkter Handschlag ohne Proxy liefert 101 und Echo.
2. `upstream.rs`: bedingte Ausnahme für das freigegebene WebSocket-Upgrade. Prüfen: `hop_by_hop_still_stripped_without_allowed_upgrade` bleibt grün, `allowed_upgrade_keeps_the_upgrade_headers` neu.
3. `handler.rs`: 101-Pfad mit `copy_bidirectional`, Spiegel, Zähler, Zustandsübergänge. Prüfen: `ws_upgrade_after_allow_echoes`.
4. Fehlerfälle: 101 ohne Upgrade, `Sec-WebSocket-Extensions` in der Antwort. Prüfen: `ws_bogus_101_is_502`.
5. Recorder und Proto-Feld, `PROTO_MINOR`. Prüfen: `recorder_stores_101_with_upgrade_and_bytes`.
6. Zeile 11 der Matrix umschreiben, CONVENTIONS 4.10 und `docs/SECURITY.md` 6, 10.3 im selben Commit auf das gebaute Verhalten bringen.

### Tests
- `allowed_upgrade_keeps_the_upgrade_headers`, `hop_by_hop_still_stripped_without_allowed_upgrade` (Unit, `upstream.rs`).
- `ws_upgrade_after_allow_echoes` (Integration): Handschlag gehalten, Allow, 101, Echo einer 1-KiB-Nachricht, Ereignisfolge `received, analyzed, held, decided, forwarded, response_headers(101), response_chunk+, recorded`.
- `ws_upgrade_blocked_stays_403`, `ws_upgrade_timeout_504`, `ws_rule_allow_skips_hold_and_upgrades` (Regel `upgrade: websocket`, `action: allow`).
- `wss_upgrade_through_connect`: dasselbe über TLS nach dem CONNECT.
- `ws_bogus_101_is_502`, `ws_extensions_are_stripped_on_the_way_up`.
- `recorder_stores_101_with_upgrade_and_bytes` (recorder).
- `conf_11_websocat_ws_upgrade_is_held` (umgeschrieben, beide Beine).

### Akzeptanzkriterien
- [ ] `cargo test -p humanitl-proxy --test conformance conf_11` grün; der Test assertet `line.code == 101` und das Echo von websocat, und `grep -n '426' daemon/crates/proxy/tests/conformance.rs` trifft nur noch den Fall für ein Upgrade, das nicht WebSocket ist.
- [ ] In der CI (websocat installiert) laufen beide Beine der Zeile 11; lokal ohne websocat wird nur das websocat-Bein übersprungen.
- [ ] Gegen einen laufenden Daemon mit einem lokalen Echo-Server (`resolver.overrides` auf `127.0.0.1`, Regel `allow_private: true`): `websocat --ws-c-uri=ws://<host>/ws - ws-c:tcp:127.0.0.1:3128` aus der Sandbox liefert nach dem Allow das Echo; `humanitl --json flows show <id>` zeigt `status: 101` und `upgrade: websocket` (Blick).
- [ ] Ohne Allow: 504 nach der Frist; mit Block: 403 mit `reason: user`; `cargo test -p humanitl-proxy` grün, die bestehenden Tests unverändert.
- [ ] `docs/SECURITY.md` 6 und 10.3 sagen genau, was nach dem Upgrade aufgezeichnet wird (Bytes Ziel → Client, Zähler Client → Ziel) und was nicht (Frames werden nicht gehalten, nicht gescannt); CONVENTIONS 4.10 nennt Zeile 11 nicht mehr als Ausnahme.
- [ ] `experimental.ws_hold` hat weiter keinen Leser (HUM-101, `pending`).
- [ ] `make check` grün, `proto/descriptor.binpb` im selben Commit, `tools/verify-commit.sh` grün.

### Fallstricke
- `Upgrade` und `Connection` sind nach RFC 9110 §7.6.1 Kopfzeilen von Verbindungsrang; die Ausnahme gilt nur für das freigegebene WebSocket-Upgrade und nur für diese Token, nie für andere `Connection`-Token.
- `copy_bidirectional` endet erst, wenn beide Richtungen geschlossen sind; Close-Frames laufen durch, ein halb geschlossener Socket darf die Task nicht ewig halten (Sitzungsende beendet sie).
- Die Bytes nach dem Upgrade umgehen die Moderation. Das ist der erklärte Seitenweg aus `docs/SECURITY.md` 10.3; der Text muss im selben Commit stimmen (`CLAUDE.md`: nichts schwächen, ohne die Sicherheitsdokumente anzupassen).
- Speicher: der Spiegel hält nur die Vorschau; `HoldMemory` (HUM-057) zählt keine aufgewertete Verbindung. Ein langer WebSocket darf den Recorder nicht mit MB-großen Chunks fluten, deshalb `recorder.max_body_bytes` als Grenze.
- `hyper::upgrade::on` auf der Client-Seite verlangt, dass die Antwort vollständig gesendet ist, bevor der aufgewertete Stream benutzt wird; die Reihenfolge im Handler ist deshalb: 101 zurückgeben, dann `on(req).await`.

### Referenzen
BACKLOG.md 2 (ADR-001 Protokoll-Ziel, ADR-007), 4.4, 9.8; `backlog/sprint-1.md` HUM-015 (Nicht-Ziel), HUM-017 (Zeile 11); `backlog/sprint-5.md` HUM-058 (Zeile 11); `docs/SECURITY.md` 6, 10.3; CONVENTIONS 3.3 (`upgrade: websocket`), 4.10; RFC 9110 §7.6.1, §7.8; RFC 6455 §4, §5; `daemon/crates/proxy/src/upstream.rs:48-58`, `tests/conformance.rs:583-672`.

---

## HUM-111 · Die Sicherheitsdokumente behaupten, was der Code nicht tut
Sprint: 1 · Größe: M · Abhängigkeiten: HUM-011, HUM-013, HUM-017, HUM-024 · Blockiert: HUM-059

### Kontext
Die Bestandsaufnahme vom 2026-09-05 hat in `docs/SECURITY.md` und `docs/THREAT-MODEL.md` dreizehn Stellen gefunden, an denen der Text etwas anderes sagt als der Code, darunter zwei Prüfbefehle, die ein Nutzer wörtlich ausführen soll und die nichts zeigen. `scripts/ci/lint-docs.sh` prüft nur Existenz, `TODO`/`TBD` und die ESC-Verweise; die „wortgleich"-Zusage aus HUM-007 für die drei Garantiesätze hat kein Gatter. HUM-059 (Sprint 5, „SECURITY.md final") ist zu weit weg für ein Dokument, dessen Prüfbefehle heute falsch sind.

| Stelle | Heute | Richtig |
|---|---|---|
| `SECURITY.md:69`, `README.md:120` | `bwrap --unshare-all --cap-drop ALL` | sechs einzelne `--unshare-*` (`bwrap_args.rs:264-266`, Snapshot `default.argv.txt:1-6`), dazu `--cap-drop ALL`, `--disable-userns`; strenger als der Text |
| `SECURITY.md:115` | Shim unter `/usr/local/bin/humanitl-shim` | `/run/humanitl/humanitl-shim` (`profiles/sandbox/default.toml:59`; `bwrap_args.rs:38-42` erklärt, warum `/usr/local/bin` unter `--ro-bind /usr` scheitert); ebenso CONVENTIONS 4.12 (`:197`) und die Doc-Kommentare `humanitl-shim/src/main.rs:18, 76` |
| `SECURITY.md:115-116` | maskierte Dateien mit `/dev/null` | versiegelte memfds über `--ro-bind-data` (`bwrap_args.rs:356-367`); `/dev/null` wurde verworfen, weil es auf einem `nodev`-Mount `EACCES` gibt |
| `SECURITY.md:125` | `find / -xdev -path /proc -prune -o -type s -print` | `-xtype s`; die Form mit `-type s` druckt in der Sandbox nichts (`tests/escape/lib.sh`, `report.rs:260-266`) |
| `SECURITY.md:364` | `hyper`/`rustls`/`rcgen` „über `hudsucker`" | hudsucker ist keine Abhängigkeit (HUM-112) |
| `SECURITY.md:438-441`, `:602-604` | gRPC über TLS scheitert „sichtbar mit `PROXY_007`" | bis HUM-108: nackter TLS-Alert; danach wahr |
| `SECURITY.md:506-513` | Audit-Log im Präsens, `humanitl audit verify` | nichts davon gebaut (`audit/src/lib.rs` 6 Zeilen, RPC `unimplemented`); geplant in HUM-050, HUM-070; `README.md:88` sagt es ehrlich |
| `SECURITY.md:557` | die argv-Zeile ist per Copy-Paste ausführbar | sie ist Anzeige: `--json-status-fd 11`, `--ro-bind-data 12..16` (`launcher.rs:170-171`); `sh -c "$(humanitl sandbox argv)"` endet 1 mit `bwrap: Write to info_fd: Bad file descriptor` |
| `SECURITY.md:568` | die Prüfungen lesen `/sys/class/net` | `/sys` ist nicht gemountet; der Shim liest `/proc/net/dev` (`report.rs:186-197`) |
| `SECURITY.md:585`, `THREAT-MODEL.md:366` | ESC-3 beweist „kein DNS vor der Entscheidung" host-seitig | die Probe ist ein `skip`; der Beweis steht in `daemon/crates/proxy/tests/dns_after_allow.rs` (HUM-115 holt ihn nach) |
| `SECURITY.md:605-606` | nach freigegebenem Upgrade fließen Frames | kein Upgrade kommt zustande, 426 (HUM-110) |
| `SECURITY.md:610` | ob `hudsucker` den Unix-Socket annimmt, „klärt ein Spike" | längst entschieden (`proxy/src/core.rs:6-18`) |

### Ziel
Jeder Befehl in `docs/SECURITY.md` 9 läuft so, wie er gedruckt ist, und zeigt, was der Text sagt. Jede Aussage im Präsens ist heute wahr oder als geplant mit Issue-Nummer gekennzeichnet. Ein Gatter hält die drei Garantiesätze in `docs/SECURITY.md` 1, `BACKLOG.md` 4.1, `app/l10n/app_de.arb` und `app_en.arb` wortgleich.

### Nicht-Ziel
Die Fähigkeiten selbst (HUM-108, HUM-110, HUM-112, HUM-115 bauen sie; dieses Issue macht die Sätze wahr, bis sie gebaut sind, und die Sätze dort ändern sich beim Bau zurück). Die Endfassung der Dokumente (HUM-059). Übersetzung (HUM-086).

### Betroffene Pfade
- `docs/SECURITY.md`, `docs/THREAT-MODEL.md`, `README.md:120`
- `backlog/CONVENTIONS.md` 4.12 (`:197`, Shim-Pfad)
- `daemon/bin/humanitl-shim/src/main.rs:18, 76` (Doc-Kommentare, Shim-Pfad)
- `scripts/ci/lint-docs.sh` (Prüfung 4: Garantiesätze), `scripts/ci/fixtures/` (Selbsttest)

### Spezifikation
Die Tabelle oben ist die Änderungsliste; jede Zeile wird eine Korrektur, keine Streichung. Ein Satz, der etwas Geplantes beschreibt, nennt das Issue in Klammern. `docs/SECURITY.md` 9.1 zeigt eine gerenderte Zeile aus `humanitl sandbox argv --profile default` (gekürzt auf die ersten Argumente und die Bindungen) mit dem Satz, dass die Zeile Anzeige ist und die Deskriptoren 11 bis 16 vom Launcher kommen.

`scripts/ci/lint-docs.sh`, Prüfung 4: liest die drei fettgedruckten deutschen Sätze aus der Tabelle in `docs/SECURITY.md` 1, die drei aus `BACKLOG.md` 4.1 und `isolationCheck1..3` aus `app/l10n/app_de.arb`; die englischen aus derselben Tabelle und `app_en.arb`. Jede Abweichung ist ein Fehler mit beiden Fassungen in der Ausgabe. Selbsttest: eine Kopie von `BACKLOG.md` mit einem geänderten Wort lässt die Prüfung 1 enden.

### Schritte
1. Prüfbefehle zuerst (`:125`, `:557`, `:568`): in einer Sandbox ausführen, Ausgabe in den Text übernehmen. Prüfen: die gedruckten Befehle liefern die gedruckte Ausgabe.
2. Mechanik-Sätze (`:69`, `:115-116`, `:364`, `:610`, `README.md:120`, CONVENTIONS 4.12, Shim-Doc-Kommentare). Prüfen: `grep -n 'unshare-all\|/dev/null\|/usr/local/bin/humanitl-shim' docs/SECURITY.md README.md backlog/CONVENTIONS.md daemon/bin/humanitl-shim/src/main.rs` ohne Treffer, die etwas Falsches behaupten.
3. Geplantes kennzeichnen (`:438`, `:506-513`, `:585`, `:602-606`, THREAT-MODEL K-10). Prüfen: jede Stelle nennt HUM-050/070, HUM-108, HUM-110 oder HUM-115.
4. Prüfung 4 in `lint-docs.sh` mit Selbsttest. Prüfen: `scripts/ci/lint-docs.sh` endet 0; die Mutation endet 1.

### Tests
- `lint_docs_guarantee_sentences_match`: Baum unverändert → Exit 0.
- `lint_docs_guarantee_sentence_drift_fails`: ein Wort in einer Kopie von `BACKLOG.md` 4.1 geändert → Exit 1, Ausgabe nennt beide Fassungen.
- Manuell in der Sandbox (`humanitl sandbox run --profile default -- sh`): die drei Befehle aus `docs/SECURITY.md` 9 liefern die im Text gezeigte Ausgabe; Ergebnis im Commit-Body.

### Akzeptanzkriterien
- [ ] `find / -xdev -path /proc -prune -o -xtype s -print` ist der gedruckte Befehl in `docs/SECURITY.md` 2, und in `humanitl sandbox run --profile default -- sh -c '<Befehl>'` druckt er genau `/run/humanitl/proxy.sock`.
- [ ] `docs/SECURITY.md` 9.1 zeigt eine gerenderte argv-Zeile und sagt, dass sie Anzeige ist; `grep -n 'Copy-Paste' docs/SECURITY.md` trifft nur noch einen Satz, der die Deskriptoren nennt.
- [ ] `grep -n 'unshare-all' docs/SECURITY.md README.md` trifft nur Sätze, die die sechs `--unshare-*` plus `--cap-drop ALL` und `--disable-userns` nennen; `grep -n '/dev/null' docs/SECURITY.md` beschreibt keine Maskierung mehr; `grep -rn '/usr/local/bin/humanitl-shim' docs backlog/CONVENTIONS.md daemon/bin/humanitl-shim/src` ist leer.
- [ ] `docs/SECURITY.md` 8 steht im Futur mit HUM-050 und HUM-070; `humanitl audit verify` ist als geplant gekennzeichnet.
- [ ] `docs/SECURITY.md` 5 und 10.2 (`PROXY_007`), 10.3 (WebSocket) und 10.5 (Listener) sagen den heutigen Stand und nennen HUM-108, HUM-110, HUM-112; `docs/THREAT-MODEL.md` K-10 nennt `dns_after_allow.rs` als Beweis und HUM-115 für die host-seitige Beobachtung.
- [ ] `docs/SECURITY.md:568` nennt `/proc/net/dev` statt `/sys/class/net`.
- [ ] `scripts/ci/lint-docs.sh` vergleicht die Garantiesätze an vier Stellen; die Mutation eines Wortes in `BACKLOG.md` 4.1 lässt es 1 enden (Ergebnis im Commit-Body); auf dem Baum endet es 0.
- [ ] `make check` grün.

### Fallstricke
- Nichts abschwächen: jede Korrektur macht den Text genauer, nicht die Aussage kleiner. Wo das Produkt strenger ist als der Text (sechs `--unshare-*`), sagt der Text das.
- Die drei Garantiesätze selbst bleiben unverändert; sie sind an vier Stellen wortgleich, und genau das prüft das neue Gatter.
- K-10 darf nicht behaupten, ESC-3 beweise etwas; bis HUM-115 steht der Beweis nur im Daemon-Test.
- `-xtype s` folgt Symlinks; der Satz daneben erklärt, warum `-type s` scheitert (Bind über eine reguläre Datei).
- HUM-108, HUM-110, HUM-112 und HUM-115 drehen die Sätze beim Bau wieder ins Präsens; wer zuerst mergt, passt an.

### Referenzen
`backlog/sprint-0.md` HUM-007; `backlog/sprint-1.md` HUM-011 (Kriterium 2, 4), HUM-013 (Kriterium 1), HUM-017 (Kriterium 3); `backlog/sprint-2.md` HUM-024 (Kriterium 3); `docs/SECURITY.md` 1, 2, 5, 8, 9, 10; `docs/THREAT-MODEL.md` K-10; `daemon/crates/sandbox/src/bwrap_args.rs:38-42, 264-266, 356-367`; `daemon/crates/sandbox/src/launcher.rs:170-171`; `tests/escape/lib.sh`; `CLAUDE.md` „Was wir nicht tun".

---

## HUM-112 · hudsucker ist keine Abhängigkeit mehr, ADR-0001 sagt das Gegenteil
Sprint: 1 · Größe: S · Abhängigkeiten: HUM-009, HUM-015 · Blockiert: HUM-059, HUM-086

### Kontext
`grep hudsucker daemon/Cargo.toml daemon/Cargo.lock` ist leer. `daemon/crates/proxy/src/core.rs:6-18` hält fest, warum: der Spike aus HUM-015 Schritt 0 fiel negativ aus (hudsucker 0.24.1 nimmt nur einen `TcpListener`), Client, Resolver und CA wären ohnehin durch die eigenen Ports ersetzt worden, und die Accept-Schleife auf hyper 1 ist kurz genug. Die Entscheidung steht nur in diesem Doc-Kommentar. Das Gegenteil behaupten `docs/adr/0001-rust-hudsucker.md` (Status `Accepted`), `docs/adr/0015-ports-and-adapters.md:27, 72, 112, 114`, `docs/ARCHITECTURE.md:15, 73`, `docs/SECURITY.md:364, 610`, `README.md:116, 245`, `AGENTS.md:21`, `BACKLOG.md:19, 63, 68, 230, 436` und `backlog/CONVENTIONS.md:71, 387`. `docs/adr/README.md:31` schreibt vor, wie eine abgelöste Entscheidung markiert wird: `Status: Superseded by ADR-XXXX`, die neue Datei verweist zurück.

### Ziel
ADR-0019 hält die Entscheidung fest: eigener Accept-Loop auf hyper 1 und `tokio-rustls` direkt auf dem Unix-Socket, hudsucker verworfen, mit dem Spike-Ergebnis als Begründung; die Rust-Hälfte von ADR-0001 („Rust, nicht mitmproxy") bleibt in Kraft und wird in ADR-0019 wiederholt, ADR-0001 trägt `Superseded by ADR-0019`. Alle Dokumente, die hudsucker als Bauteil nennen, nennen stattdessen hyper 1, rustls, rcgen und die drei Ports (`Egress`, `Resolver`, `LeafCache`).

### Nicht-Ziel
Code. Übersetzung (HUM-086). Die Endfassung von README und SECURITY (HUM-059).

### Betroffene Pfade
- `docs/adr/0019-hyper-accept-loop.md` (neu), `docs/adr/0001-rust-hudsucker.md` (Status), `docs/adr/README.md` (Index)
- `docs/adr/0015-ports-and-adapters.md`, `docs/ARCHITECTURE.md`, `docs/SECURITY.md:364, 610`, `README.md:116, 245`, `AGENTS.md:21`
- `BACKLOG.md:19, 63, 68, 230, 436` (ADR-001-Kurzform, Stack-Tabelle, Monorepo-Layout, HUM-015-Zeile), `backlog/CONVENTIONS.md:71, 387`, `backlog/sprint-5.md:74` (HUM-057, „hudsucker-Optionen")
- `daemon/crates/proxy/src/core.rs:6-18` (Doc-Kommentar verweist auf ADR-0019)

### Spezifikation
ADR-0019 nach `docs/adr/0000-template.md` (sieben MADR-Überschriften, `Status: Accepted`, `Datum`): Kontext (Spike-Ergebnis aus `core.rs`), Entscheidung (Accept-Loop auf `UnixListener`, hyper 1 `http1::Builder` mit `with_upgrades`, `tokio-rustls`, `rcgen` über `LeafCache`), Konsequenzen (kein TCP-Port auf dem Host, drei Ports statt einer Bibliothek, was bei einem Wechsel zu tun wäre), Betroffene Issues (HUM-015, HUM-017, HUM-023, HUM-024, HUM-045). ADR-0001 behält Titel und Text, bekommt `Status: Superseded by ADR-0019` und einen Satz am Kopf, dass die Rust-Entscheidung in ADR-0019 fortgilt. `docs/adr/README.md` bekommt die Zeile und den Status-Wechsel.

Textstellen: „hudsucker-Wrapper" wird „Proxy-Kern auf hyper 1" (`ARCHITECTURE.md:15`, `BACKLOG.md:230, 436`); die Stack-Tabelle `BACKLOG.md:19` nennt „hyper 1 + rustls + rcgen, eigener Accept-Loop (ADR-0019)"; `AGENTS.md:21` „hyper-based MITM proxy"; `README.md:116` „A MITM proxy built on hyper and rustls"; `SECURITY.md:364` streicht „(über `hudsucker`)", `:610` nennt die Entscheidung statt des Spikes. `CONVENTIONS.md:71` streicht `hudsucker 0.25 mit Features ...` aus der Stack-Zeile; `:387` bleibt als Protokoll des Spikes und bekommt den Verweis auf ADR-0019.

### Schritte
1. ADR-0019 schreiben, ADR-0001 umstellen, Index. Prüfen: `bash docs/adr/check.sh` meldet 19 ADRs, konsistent.
2. Textstellen nachziehen. Prüfen: `grep -rn hudsucker README.md AGENTS.md docs/ARCHITECTURE.md docs/SECURITY.md docs/adr/0015-ports-and-adapters.md BACKLOG.md backlog/CONVENTIONS.md backlog/sprint-5.md` trifft nur historische Nennungen (ADR-0001, CONVENTIONS 4.10 Spike-Protokoll, BACKLOG ADR-001-Kurzform als abgelöst markiert).
3. `core.rs` Doc-Kommentar verweist auf ADR-0019. Prüfen: `cargo doc -p humanitl-proxy --no-deps` ohne Warnung.

### Tests
- `docs/adr/check.sh` (bestehend): Nummerierung, Index, Status-Vokabular, sieben Überschriften.
- `scripts/ci/lint-docs.sh` grün.
- Der grep aus Schritt 2 als Kriterium.

### Akzeptanzkriterien
- [ ] `docs/adr/0019-hyper-accept-loop.md` existiert, `bash docs/adr/check.sh` meldet `19 ADRs, template and index are consistent`.
- [ ] `grep -n '^Status:' docs/adr/0001-rust-hudsucker.md` liefert `Status: Superseded by ADR-0019`, und ADR-0019 nennt ADR-0001 im Kontext.
- [ ] `grep -rn hudsucker README.md AGENTS.md docs/ARCHITECTURE.md docs/SECURITY.md docs/adr/0015-ports-and-adapters.md` ist leer; in `BACKLOG.md` und `backlog/CONVENTIONS.md` trifft es nur Stellen, die die Ablösung oder das Spike-Protokoll nennen.
- [ ] `grep -n 'ADR-0019' daemon/crates/proxy/src/core.rs` trifft im Doc-Kommentar.
- [ ] `make check` grün.

### Fallstricke
- Die Entscheidung „Rust, nicht mitmproxy" darf nicht mit abgelöst werden; ADR-0019 wiederholt sie ausdrücklich.
- `BACKLOG.md` 2 ist die Kurzform der ADRs; die Überschrift „ADR-001 Daemon in Rust auf hudsucker" wird umbenannt, der Absatz sagt in einem Satz, was ADR-0019 ersetzt hat. Nichts löschen, was die Entscheidung erklärt.
- `check.sh` kennt nur `Accepted`, `Superseded by ADR-NNNN`, `Deprecated`; keine eigene Schreibweise erfinden.

### Referenzen
`docs/adr/README.md` (Regeln, Index), `docs/adr/0000-template.md`, `docs/adr/0001-rust-hudsucker.md`, `docs/adr/0015-ports-and-adapters.md`; `daemon/crates/proxy/src/core.rs:6-18`; `backlog/CONVENTIONS.md` 4.10 (Spike); `backlog/sprint-1.md` HUM-015 Schritt 0; BACKLOG.md 2, 3.2.

---

## HUM-113 · `escape-launch` ist verwaist
Sprint: 1 · Größe: S · Abhängigkeiten: HUM-064 · Blockiert: keine

### Kontext
`daemon/crates/sandbox/src/bin/escape-launch.rs` (1111 Zeilen) war der Starter des Escape-Harness aus HUM-006. Seit HUM-064 starten alle Suiten über `humanitl sandbox run --profile test` (`tests/escape/run.sh:20` sagt es aus, `:316` tut es); `grep -rn escape-launch Makefile .github scripts` findet keinen Aufruf. Das Binary wird weiter mit `cargo build --workspace` übersetzt, trägt neun Unit-Tests und eine zweite Kopie der Logik „Bericht abwarten, Garantien prüfen, bei Rot beenden", die `docs/SECURITY.md:629-631` und `backlog/CONVENTIONS.md:476` als einen der Wege in die Sandbox nennen, die sich gleich verhalten müssen. Zwei Kopien einer Sicherheitslogik, von denen eine niemand mehr ausführt, driften. (`SANDBOX_010` bleibt: `daemon/bin/humanitl/src/cmd/sandbox.rs:371` baut ihn.)

### Ziel
Es gibt einen Testweg in die Sandbox, `humanitl sandbox run`. Das Binary ist gelöscht, die Behauptungen seiner Tests, die der CLI-Weg noch nicht deckt, sind dorthin gewandert, und kein Dokument nennt `escape-launch` mehr als Weg.

### Nicht-Ziel
Änderungen an `humanitl sandbox run` über die übernommenen Tests hinaus. Die Escape-Suiten selbst.

### Betroffene Pfade
- `daemon/crates/sandbox/src/bin/escape-launch.rs` (gelöscht), `daemon/crates/sandbox/Cargo.toml` (falls ein `[[bin]]`-Eintrag entsteht oder entfällt)
- `daemon/bin/humanitl/tests/cli.rs` (übernommene Tests)
- `tests/escape/README.md:79, 171`, `docs/SECURITY.md:625-631` (Tabelle unter 10, Punkt 6b), `backlog/CONVENTIONS.md:476`
- `backlog/sprint-0.md` HUM-006 (Vermerk im Kopf des Issues, dass der Starter durch HUM-064 ersetzt wurde)

### Spezifikation
Vor dem Löschen: die neun Tests des Binaries gegen `daemon/bin/humanitl/tests/cli.rs` und `daemon/crates/sandbox/tests/launcher.rs` abgleichen. Jede Behauptung, die dort fehlt (Beispiel: Platzhalter-Socket unter dem erwarteten Pfad, leere CA-Datei unter `<state>`, `SANDBOX_010` bei fremder Argumentform), wird als Test am CLI-Weg neu geschrieben oder mit einem Satz im Commit-Body als durch die Suiten abgedeckt erklärt. Danach Datei löschen. die Tabelle unter `docs/SECURITY.md` 10, Punkt 6b, hat zwei Zeilen (Daemon, `humanitl sandbox run`); CONVENTIONS `:476` beschreibt das Verhalten für `humanitl sandbox run`.

### Schritte
1. Testabgleich, Ergebnis als Liste im Commit-Body. Prüfen: jede der neun Behauptungen hat einen Ort oder einen Satz.
2. Datei löschen, Workspace bauen. Prüfen: `cargo build --workspace --all-targets` grün, `cargo test -p humanitl-sandbox` grün mit entsprechend weniger Tests.
3. Dokumente nachziehen. Prüfen: grep unten.
4. Escape-Harness fahren. Prüfen: gleiche Zahl grüner Fälle wie vorher.

### Tests
- Bestehende Suiten: `./tests/escape/run.sh` mit derselben Bilanz (heute 104 Fälle, 96 grün, 8 übersprungen).
- Übernommene Tests in `cli.rs`, benannt im Commit-Body.

### Akzeptanzkriterien
- [ ] `test -e daemon/crates/sandbox/src/bin/escape-launch.rs` schlägt fehl; `cargo build --workspace --all-targets` grün.
- [ ] `grep -rn 'escape-launch' --exclude-dir=target --exclude-dir=.git --exclude-dir=.claude .` trifft nur `backlog/` (Historie) und keinen Satz, der ihn als Weg beschreibt.
- [ ] die Tabelle unter `docs/SECURITY.md` 10, Punkt 6b, hat zwei Zeilen; `backlog/CONVENTIONS.md:476` nennt `humanitl sandbox run`.
- [ ] `./tests/escape/run.sh` endet mit derselben Zahl grüner Fälle wie vor dem Commit (Zahlen im Commit-Body).
- [ ] Der Commit-Body listet die neun Tests des Binaries und sagt zu jedem, wo seine Behauptung jetzt lebt.

### Fallstricke
- Nicht löschen, bevor der Abgleich steht; die neun Tests sind die einzige Beschreibung dessen, was das Binary konnte.
- `SANDBOX_010..012` bleiben registriert und gebaut (CLI); nicht mit HUM-108s Lint verwechseln.
- `tests/escape/README.md:79` erklärt Geschichte; ein Satz Geschichte darf bleiben, aber nicht als Anleitung.

### Referenzen
`backlog/sprint-0.md` HUM-006; `backlog/sprint-1.md` HUM-064; `tests/escape/run.sh:19-21, 316`; `docs/SECURITY.md` 10 (Punkt 6b); `backlog/CONVENTIONS.md:476`; `daemon/crates/sandbox/src/bin/escape-launch.rs:1-30`.
