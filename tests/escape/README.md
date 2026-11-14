# Escape-Tests

Die Escape-Tests sind der messbare Teil des Sicherheitsversprechens. Sie starten
eine Sandbox nach `profiles/sandbox/test.toml` und versuchen darin genau das, was
nie gelingen darf. Die Aussagen, die sie prüfen, stehen in `docs/SECURITY.md` und
`docs/THREAT-MODEL.md`; hier steht, wie man sie ausführt und was ihr Ergebnis in
Sprint 0 bedeutet.

```
make escape                       # ganze Suite, rot wenn eine Probe durchkommt
ESCAPE_ALLOW_FAIL=1 make escape   # berichten, aber den Build nicht rot färben
./tests/escape/selftest.sh        # nur das Harness gegen sich selbst prüfen
```

Ergebnisse: `target/escape/escape.xml` (JUnit), `target/escape/results.txt` (roh,
eine Zeile je Probe), `target/escape/esc-N.log` (Ausgabe des jeweiligen Laufs).

## Je Backend (HUM-435)

Ein Backend ist erst zulässig, wenn diese Suite gegen es grün ist (BACKLOG.md
9.1). Dieselben Skripte laufen deshalb gegen jedes Backend;
`HUMANITL_ESCAPE_BACKEND` wählt es, Vorgabe `bwrap`:

```
./tests/escape/run.sh                                  # bwrap
HUMANITL_ESCAPE_BACKEND=podman ./tests/escape/run.sh   # Podman, gemeinsame Stufe
HUMANITL_ESCAPE_BACKEND=podman HUMANITL_ESCAPE_IMAGE=docker.io/library/debian@sha256:... \
    ./tests/escape/run.sh                              # Podman mit Abbild als Wurzel
```

Mit `HUMANITL_ESCAPE_IMAGE` (HUM-439) ist die Wurzel jeder Sandbox des Laufs
dieses lokale Abbild (`escape-launch --image`); gezogen wird es nie, ein
fehlendes ist `SANDBOX_043` und damit `sandbox_not_started`. Unter bwrap ist
die Variable ein Fehler des Laufs. Vor den Suiten schreibt `run.sh` Gerät und
Inode der CA des Daemons, der `/etc/shadow` und der `/etc/machine-id` des
Hosts nach `/work/results/host-ids` (`escape_host_ids` in `launch.sh`):

| Fall | Behauptung |
|---|---|
| `esc-2/ca_is_the_hosts` | die CA unter `/etc/humanitl/ca.crt` ist die Datei des Daemons, nach Gerät und Inode, unter jedem Backend und unter einem Abbild |
| `esc-2/ca_bundle_is_the_hosts` | das Bündel unter `/etc/ssl/certs/ca-certificates.crt` ist die Datei des Daemons, nach Gerät und Inode, unter jedem Backend und unter einem Abbild |
| `esc-2/no_etc_shadow`, `esc-2/no_machine_id` | ohne Abbild gibt es beide nicht; unter einem Abbild bringt es sein eigenes `/etc` mit, und dann darf keine der beiden die Datei des Hosts sein (Gerät und Inode) |

Das Abbild für diesen Lauf baut `tests/escape/build-image.sh` (HUM-455) aus
`tests/escape/image/Containerfile`: eine per Digest festgelegte Debian-Basis
und darüber jeder Helfer, den die Suiten in der Sandbox aufrufen (`python3`,
`curl`, `stat`, `id`, `find`, `getent` und die übrigen; fehlt einer, scheitert
schon der Bau). Das Skript prüft den Verweis so, wie der Adapter ihn vor dem
Start prüft, und gibt ihn als letzte Zeile aus:

```
ref=$(./tests/escape/build-image.sh) &&
    HUMANITL_ESCAPE_BACKEND=podman HUMANITL_ESCAPE_IMAGE="$ref" ./tests/escape/run.sh
```

Der CI-Job `escape-tests-podman` fährt die Suiten zweimal: erst mit leerer
Wurzel, dann mit diesem Abbild, beide mit `ESCAPE_NO_SKIP=1`. Gebaut wird vor
dem Start des Daemons und vor der gepflanzten Engine-Konfiguration, außerhalb
jeder Sandbox; der Lauf mit dem Abbild zieht nichts. Die zwei Starts von
`mount-proof.sh` (`swapped_folder`, `unplanned_mount`) nehmen das Abbild nicht:
Sie prüfen den Abgleich von Plan und Start, nicht die Wurzel.

Der Weg in die Sandbox steht in `launch.sh` (`escape_sandbox_run`):

| Backend | Starter | Warum |
|---|---|---|
| `bwrap` | `humanitl sandbox run --profile test` | der Weg, den der Nutzer nimmt (CONVENTIONS 3.11) |
| jedes andere | `escape-launch --backend <name>` | eine Stufe vor ihrer Zulassung lehnt `humanitl sandbox run` mit `SANDBOX_048` ab; nur die Suite und die Tests starten sie, über ein Argument im Code |

`escape-launch` gibt es nur mit dem Cargo-Feature `unadmitted-backends`
(`required-features`), das weder `humanitld` noch `humanitl` je einschalten
(`humanitl_sandbox::UNADMITTED_BACKENDS_BUILT`, `tests/feature_gate.rs`). `run.sh`
baut es in einem eigenen Cargo-Aufruf, weil Cargo Features über alle Pakete
eines Aufrufs vereinigt. Der Starter bekommt Proxy-Socket, CA und Bundle des
Daemons dieses Laufs, ESC-3 misst also denselben Proxy, und druckt nach dem Lauf
die verweigerten Sockets in denselben Zeilen wie die Kommandozeile
(`Refusal::summary_line`), damit `refusals-proof.sh` unter jedem Backend dieselbe
Zeile liest. Er behält `HOME` und `XDG_RUNTIME_DIR` des Nutzers: Die
Container-Engine hat ihren Speicher unter `HOME`, und ein zweiter Speicher
neben dem des Nutzers beschädigte dessen Zustand. Mit HUM-448 wird Podman
zugelassen; dann kann `run.sh` es über `humanitl sandbox run --backend podman`
starten, und `escape-launch` bleibt für die Kernel-Stufe bis HUM-449.

Ein Backend, das sich nicht starten lässt (nicht installiert, zu alt, nicht
rootless), hinterlässt keine Ergebnisdatei und ist `error`
(`sandbox_not_started`, Exit 2), nie `pass` oder `skip`. `selftest.sh` prüft
das gegen Attrappen beider Starter, bevor `run.sh` sie aufruft: Mit
`HUMANITL_ESCAPE_BACKEND=podman` geht die Suite über `escape-launch` mit diesem
Backend und dem Socket des Daemons, nie über die Kommandozeile, und ein
Starter, der mit 3 endet, steht als Fehler im Bericht.

Keine Probe entfällt für ein Backend. Was eine Container-Engine zusätzlich
mitbringt, prüft ESC-2 im Abschnitt „the container engine", und jede dieser
Proben läuft auch unter bwrap, wo sie hält, weil es nichts davon gibt:

| Fall | Behauptung |
|---|---|
| `esc-2/containerenv_empty` | `/run/.containerenv` ist leer oder fehlt; ohne `--privileged` schreibt Podman nichts hinein, außer `graphRootMounted=1`, wenn sein Speicher eingehängt ist |
| `esc-2/no_engine_socket` | kein Socket von `conmon` (Attach, Steuerung) und keiner der Engine, weder als Datei noch in `/proc/net/unix` des Netz-Namensraums |
| `esc-2/pid1_shim_seccomp_2` | PID 1 ist der Shim, mit `Seccomp: 2`, im selben Prozess; kein Init der Engine davor |
| `esc-2/no_engine_secrets`, `esc-2/no_engine_volume` | ein Eintrag in der `mounts.conf` des Nutzers (`/run/secrets`) und ein Volume aus seiner `containers.conf` erreichen die Sandbox nicht |
| `esc-2/engine_hook_not_run` | ein OCI-Hook aus der Konfiguration des Nutzers läuft für keine Sandbox des Laufs (auf dem Host, `engine-config-proof.sh`) |
| `esc-2/swapped_folder` | ein Projektordner, zwischen Plan und Start getauscht, erreicht den Befehl nie: der Start endet mit `SANDBOX_042`, oder die Sandbox sieht den geprüften Ordner (auf dem Host, `mount-proof.sh`, HUM-437) |
| `esc-2/unplanned_mount` | eine Einhängung in der Zeile, die der Plan nicht erwartet, beendet den Start mit `SANDBOX_049`, bevor der Befehl läuft (auf dem Host, `mount-proof.sh`, HUM-437) |

Die beiden letzten Zeilen brauchen eine Konfiguration, die jemand gepflanzt
hat. Das tut `plant-engine-config.sh` im CI-Job `escape-tests-podman`, und nur
dort: Es schreibt nach `~/.config/containers`, verweigert den Lauf außerhalb
eines CI-Runners und überschreibt nie eine vorhandene Datei. Vor dem Lauf zeigt
es mit einem gewöhnlichen Container aus einem lokal importierten Abbild, dass
die Engine Eintrag, Volume und Hook wirklich anwendet; sonst wären die Proben
grün für eine Konfiguration, die niemand gelesen hat. Ohne gepflanzte
Konfiguration sind `no_engine_secrets` und `no_engine_volume` trivial grün, und
`engine_hook_not_run` ist für ein anderes Backend als bwrap ein `skip`, der es
sagt.

Der CI-Job `escape-tests-podman` (`ubuntu-24.04`) hat dieselben Schritte und
dasselbe Urteil (`scripts/ci/escape-verdict.sh`) wie `escape-tests`, dazu
`podman --version` und `podman info` als Beleg (P16), die gepflanzte
Konfiguration und die Live-Tests des Adapters
(`cargo test -p humanitl-sandbox --lib podman -- --test-threads=1` mit
`HUMANITL_REQUIRE_PODMAN=1`, das ein fehlendes Podman zum Fehlschlag macht).
ESC-4 und ESC-5 laufen auf dem Host. ESC-4 hängt nicht vom Backend ab und läuft
mit, damit der Job dieselbe Bilanz hat wie `escape-tests`. ESC-5 dagegen startet
in seinen Dateisystem- und Terminal-Fällen selbst eine Sandbox, und zwar die des
Laufs: `run.sh` reicht das Backend als `ESC_BACKEND` an
`esc-5-filesystem.sh`, das es den Integrationstests als
`HUMANITL_ESCAPE_BACKEND` gibt und sie für ein anderes Backend als bwrap mit
`--features unadmitted-backends` baut, in einem eigenen Cargo-Aufruf je Paket.
`escape_worktree.rs` holt sich das Backend dann über
`Backend::detect_unadmitted`, `terminal.rs` startet den Dienst des Daemons mit
genau diesem Backend (`SandboxPorts::with_backend`, `sandbox.backend` nennt
dasselbe). Fehlt das verlangte Backend, ist der Fall rot, immer: nie ein
`skip`, und still gegen bwrap läuft er nie. Der Fall zur Wegwerf-Sandbox hat zwei Fassungen: unter bwrap die Messung am
Overlay (`tests/launcher.rs`), unter einem Backend ohne Wegwerf-Sandbox (Podman,
siehe HUM-438) die Prüfung, dass es den Plan verweigert, statt direkt in das
Projekt zu starten (`escape_worktree.rs`, rot, sobald das Backend eine
Wegwerf-Sandbox kann). Nur die zwei Audit-Fälle hängen nicht vom Backend ab: Sie
starten einen Daemon, schicken einen Fluss durch seinen Proxy und starten dabei
keine Sandbox.

## Die Kernel-Stufe (HUM-445)

```
HUMANITL_ESCAPE_BACKEND=qemu ./tests/escape/run.sh     # QEMU-micro-VM, Kernel-Stufe
```

Die Kernel-Stufe (Backend `qemu`, ADR-0021) ist bis HUM-449 nicht zugelassen;
`run.sh` startet sie wie Podman vor HUM-448 über `escape-launch --backend qemu`
und damit über `Backend::detect_unadmitted`. Alle Suiten laufen, auch die
Mounttabelle aus HUM-437. Gebraucht werden `/dev/kvm`, ein Dienstverwalter des
Nutzers und die Pakete aus `scripts/ci/install-kernel-level.sh`; den Gastkernel
nehmen `escape-launch` und die Fälle von ESC-5 aus `HUMANITL_TEST_VM_KERNEL`
und `HUMANITL_TEST_VM_MODULES`, wo sie gesetzt sind
(`humanitl_sandbox::with_test_guest_kernel`, nur mit dem Feature der
Umgehung), sonst aus `/boot`. Der Arbeitsbaum braucht einen kurzen Pfad: Der
Socket des Daemons des Laufs liegt unter `target/escape/state/runtime` und
trägt höchstens 107 Byte.

Die Ausgabe des Befehls reist unter dieser Stufe nur als Strom (HUM-443);
`escape-launch` startet sie deshalb mit Pseudoterminal und Strom und gibt die
Ausgabe auf seine Standardausgabe, damit `mount-proof.sh` sie liest wie unter
jedem anderen Backend. Nach dem Start schreibt es `escape-launch: session
<uuid>`, damit `kernel-proof.sh` die Unit der Sitzung findet.

### Wie unter bwrap, durch den Adapter

Der Gast sieht von innen aus wie eine bubblewrap-Sandbox, und die Proben von
ESC-1 und ESC-2 dazu gelten unverändert: Der Agent und jede weitere Shell
geben vor dem Filter jede Fähigkeit ab und laufen als der Nutzer, den bwrap
abbildet (`caps_effective_empty`, `caps_bounding_empty`, `pid1_not_dumpable`,
`pid1_mem_rdwr` und die Fälle des Shim-Elternteils); `/sys` ist nach der
Modulsperre ein leeres, nur lesbares tmpfs (`no_host_sys`, `no_sys_content`);
`/dev/shm` ist ein tmpfs (`shm_is_tmpfs`); `/run`, `/tmp`, `/var/tmp` und
das Heimatverzeichnis des Agenten sind beschreibbare tmpfs wie unter bwrap,
`/run/humanitl` bleibt darüber nur lesbar (`writable_places_kernel`, HUM-466); der Rechnername ist `sandbox`
(`hostname_sandbox`). `esc-2/engine_hook_not_run` ist unter qemu wie unter
bwrap kein Fall: Die Stufe liest keine Konfiguration einer Container-Engine.

### Kernel-Varianten

Wo die physische Tatsache im Gast eine andere ist, die Behauptung aber
dieselbe, hat ESC-2 eine Kernel-Variante. `run.sh` schreibt dafür
`level kernel` in `/work/results/host-ids`, dazu SHA-256 der CA und des
Bündels und Gerät und Inode des `.git/hooks` des Projekts. Jede Variante prüft
die Behauptung mindestens so streng wie der Fall unter bwrap:

| bwrap | Kernel-Variante | warum gleichwertig |
|---|---|---|
| `ca_is_the_hosts`, `ca_bundle_is_the_hosts`: Gerät und Inode gleich der Datei des Daemons | `ca_is_the_hosts_kernel`, `ca_bundle_is_the_hosts_kernel`: Inode gleich, die Datei liegt auf der virtiofs-Freigabe, Inhalt nach SHA-256 gleich | virtiofs reicht den Inode durch, die Gerätenummer vergibt der Gast selbst (HUM-442 Q5); fs-Typ und Inhalt schließen eine andere Datei mit zufällig gleichem Inode aus |
| `exactly_one_socket`, `socket_is_proxy`, `proxy_is_socket_file`: genau ein Unix-Socket, der Proxy | `socket_none_kernel`: kein einziger Unix-Socket an den Orten, an denen im Gast einer stehen kann (`/work`, `/dev` mit `/dev/shm`, `/tmp`, `/var/tmp`, `/run`, `/home`), bei vollständigem Durchlauf, mit einem in `/dev/shm` gepflanzten Socket-Knoten als Positivkontrolle, den der Durchlauf finden muss; der Rest ist die nur lesbare Freigabe des Plans, in der `single_socket` und die Host-Prüfung keinen Socket zulassen (ein Durchlauf über das ganze `/usr` des Hosts erschöpft die Deskriptoren des Dateisystemdienstes); `door_is_loopback_kernel`: die Proxy-Variablen nennen nur `http://127.0.0.1:3128`; `door_reaches_proxy_kernel`: diese Tür antwortet als Proxy (Positivkontrolle) | die Tür ist im Gast keine Datei, sondern ein vsock-Port des Hosts hinter der Brücke des Shims; `AF_VSOCK` und `AF_UNIX` verweigert der Filter des Agenten (ESC-1), der einzige Weg ist also `127.0.0.1:3128`, und dahinter steht der Proxy |
| `proxy_socket_dir_is_not_host`: in `/run/humanitl` nichts außer Proxy-Socket und Shim (grün auch, wenn `ls` scheitert) | `proxy_socket_dir_is_not_host_kernel`: `ls -A /run/humanitl` gelingt und nennt nur `humanitl-shim`, `guest-plan.json` und `modules` | im Gast liegen dort statt des Sockets der Gastplan und die Module des Gastkernels; die Behauptung ist, dass weder Socket noch Token des Daemons dort stehen (`no_daemon_socket` unverändert), und die Variante ist strenger, weil ein scheiterndes `ls` rot ist |
| `git_hooks_is_tmpfs`: ein tmpfs über `.git/hooks` | `git_hooks_empty_kernel`: leer; `git_hooks_not_host_kernel`: anderer Inode als das Verzeichnis des Hosts; `git_hooks_write_kernel` und auf dem Host `git_hooks_write_stays_kernel`: ein dort geschriebener Hook fehlt nach dem Lauf im Projekt | das tmpfs liegt im Gefängnis des Dateisystemdienstes von `/work`, der Gast sieht die Freigabe; die Behauptung der Maske ist, dass nichts vom Host zu sehen ist und nichts zum Host gelangt, und das wird direkt gemessen |
| `seccomp_every_process`: jeder Prozess `Seccomp: 2` | derselbe Fall unter jedem Backend, Kernel-Threads ausgenommen | ein Kernel-Thread führt keinen Code des Nutzerraums aus; erkannt allein an `PF_KTHREAD` in Feld 9 von `/proc/<pid>/stat`, das nur der Kernel setzt, nie am Elternprozess oder am Namen; `selftest.sh` prüft das an einem Baum mit beidem |

### Root im Gast (`guest-root.sh`, `kernel-proof.sh`)

Die Garantien der Stufe liegen auf dem Host und müssen auch dann halten, wenn
der Filter fällt und der Agent Root in seinem Kernel wird. Dafür startet der
Shim im Gast neben dem Agenten einen Befehl als Root ohne seccomp-Filter: den
Schlüssel `root_probe` des Gastplans, den der Daemon nur schreibt, wenn
`LaunchOptions::guest_root_probe` gesetzt ist. `GuestRootProbe::new` gibt es
nur mit dem Feature `unadmitted-backends`, also nur in `escape-launch`
(`--test-guest-root-probe`) und in Tests; wie `SocketFloor` ein Argument im
Code, nie ein Schlüssel. Der Bericht trägt dann eine Zeile `ROOT-PROBE`.

| Fall | Behauptung |
|---|---|
| `esc-guest/root_without_filter` | die Voraussetzung: uid 0, `Seccomp: 0`, `CAP_SYS_MODULE` und `CAP_NET_ADMIN` wirksam; ohne sie mäßen die Proben nur den Filter |
| `esc-guest/only_lo` | im Gast gibt es nur `lo` |
| `esc-guest/no_route_out_inet`, `_inet6`, `_slirp` | `connect` nach außen scheitert mit einem errno, auch zum Gateway eines Nutzernetzes von QEMU |
| `esc-guest/vsock_door_reaches_host` | Positivkontrolle: CID 2, Port 5000, die Tür, nimmt die Verbindung an |
| `esc-guest/vsock_other_ports` | jeder andere Port des Hosts weist ab, auch 5001 (Bericht und Steuerkanal, nur vom Host geöffnet) und 5002 bis 5010 neben der Tür |
| `esc-guest/vsock_other_cids` | jede andere CID weist ab, auch die eigene und die des Hypervisors |
| `esc-guest/vsock_sibling_guest` | ein zweiter Gast einer anderen Sitzung, der auf Port 6000 lauscht, ist unter keiner CID erreichbar (HUM-442 Q9) |
| `esc-guest/sibling_listening`, `sibling_not_reached`, `sibling_outlived_the_probe` | der zweite Gast lauschte als Root ohne Filter, nahm keine Verbindung an und lauschte noch, als der erste fertig war |
| `esc-guest/modules_locked`, `finit_module_locked`, `modules_lock_one_way` | `kernel.modules_disabled` ist 1, `finit_module` eines noch nicht geladenen Moduls (`dummy`) endet mit `EPERM`, und die Sperre lässt sich nicht zurücksetzen |
| `esc-guest/host_unit_only_lo` | auf dem Host sieht jeder Prozess der Unit der Sitzung nur `lo`; der Beleg nennt die vier Hilfsprogramme, damit er die richtige Unit misst |

Jeder Versuch über vsock setzt `SO_VM_SOCKETS_CONNECT_TIMEOUT` auf eine halbe
Sekunde: Ein Ziel, das es nicht gibt, beantwortet der Gast mit einem Timeout
statt einer Ablehnung (HUM-442). Geurteilt wird an den Zeilen, nie am
Exit-Code: Jedes verlangte Ziel muss genau eine Zeile `<cid>:<port> E…`
haben, und es darf keine weitere geben; ein Helfer, der bricht oder ein Ziel
auslässt, ist rot.

Der Fall zur Wegwerf-Sandbox in ESC-5 ist unter qemu die Prüfung der
Verweigerung (`SANDBOX_030`), wie unter Podman vor HUM-438; er wird rot,
sobald HUM-446 die Wegwerf-Schicht liefert, und braucht dann die echte
Messung. Die Dateisystem-Fälle von ESC-5 starten unter dieser Stufe mit
Strom und Pseudoterminal und lassen den Agenten durch die Prüfung des Hosts
zu (`isolation_check`), wie der Daemon es tut.

Der CI-Job `escape-tests-qemu` (`ubuntu-24.04`) baut auf `kvm-tests` auf:
dieselbe udev-Regel für `/dev/kvm`, dieselbe Freigabe der
Nutzer-Namensräume, `loginctl enable-linger` für den Dienstverwalter, dieselben
festgelegten Debian-Pakete und der Gastkernel. Er fährt `run.sh` mit
`ESCAPE_NO_SKIP=1` im Urteil; ein fehlendes `/dev/kvm` ist rot.

## Rot ist in Sprint 0 der richtige Zustand

Das Harness ist vor der Sache geschrieben, die es bewacht (BACKLOG.md 4.5,
Risiko 1). Der Launcher (HUM-011), der Shim mit seinem seccomp-Filter (HUM-012)
und der Proxy (HUM-013/015) existieren noch nicht. Jede Probe, die von ihnen
abhängt, ist heute rot — mit dem Beleg, der sie rot macht, in der Ausgabe. Das ist
der Sinn der Übung: die Aussage ist ab der ersten Zeile des Filters messbar, und
niemand muss dem Filter glauben.

Seit HUM-021 läuft der CI-Job `escape-tests` ohne `ESCAPE_ALLOW_FAIL`: Eine rote
Probe färbt den Build rot. Der Schalter bleibt für lokale Läufe, die den Bericht
ohne das Urteil wollen.

## Exit-Codes

| Code | Bedeutung |
|---|---|
| 0 | jede Probe grün oder übersprungen (oder `ESCAPE_ALLOW_FAIL=1`) |
| 1 | die Sandbox lief, und mindestens eine Probe ist durchgekommen |
| 2 | die Sandbox ließ sich gar nicht starten, oder der Selbsttest scheiterte |

Der Unterschied zwischen 1 und 2 ist der wichtigste im Bericht: „kein `bwrap` auf
dieser Maschine" ist eine Aussage über die Maschine, keine über die Garantie.
`ESCAPE_ALLOW_FAIL=1` mildert nur die 1, nie die 2. Ein nicht startbarer Lauf
erscheint im XML als `<error>`, eine durchgekommene Probe als `<failure>`.

## Aufbau

| Datei | Rolle |
|---|---|
| `run.sh` | Einstieg, was `make escape` aufruft: baut Kommandozeile, Daemon und Shim, fährt die Suiten, schreibt das XML. Die Binaries werden aus `${CARGO_TARGET_DIR:-daemon/target}/debug/` gelesen, demselben Verzeichnis, in das `cargo build` sie legt (ein relatives `CARGO_TARGET_DIR` zählt ab `daemon/`) |
| `lib.sh` | die Proben-Helfer, in Sandbox und auf dem Host dieselben |
| `junit.sh` | aus den `RESULT`-Zeilen wird JUnit-XML |
| `selftest.sh` | prüft `lib.sh` und `junit.sh` gegen `true` und `false`, und die Socket-Probe gegen echte Sockets (schlicht, unter `dev/shm`, per Bind-Mount) |
| `esc-1-sockets.sh` | ESC-1: Socket-Familien und -Typen, Interfaces, Routing, Capabilities, seccomp |
| `esc-2-mounts.sh` | ESC-2: Mount-Oberfläche, genau ein Socket, eigene Namespaces, Maskierungen |
| `esc-3-egress.sh` | ESC-3: kein Egress ohne Proxy, über den Proxy landet alles in der Warteschlange; am Ende die zwei Anfragen des DNS-Beweises (`held.esc3.test`, `allowed.esc3.test`) |
| `dns-stub.py` | der aufzeichnende Nameserver des Laufs (HUM-115): UDP auf einem freien Port von `127.0.0.1`, eine Zeile `<epoch-ms> <qname> <qtype>` je Frage nach `target/escape/dns.log`, Antwort immer `NXDOMAIN` |
| `dns-proof.sh` | von `run.sh` eingelesen: startet den Stub, lässt neben ESC-3 einen Beobachter auf dem Host laufen und entscheidet die drei Fälle `esc-3/dns_not_before_decision`, `esc-3/dns_after_allow_once`, `esc-3/meta_no_dns_lookup` am Protokoll des Stubs |
| `refusals-proof.sh` | von `run.sh` eingelesen: startet nach den Suiten eine weitere Sandbox, lässt sie genau dreimal `socket(AF_UNIX)` und zweimal `socket(AF_INET, SOCK_DGRAM)` versuchen und entscheidet `esc-1/refusals_reported` an dem, was `humanitl sandbox run` danach über die verweigerten Versuche sagt (HUM-138) |
| `mount-proof.sh` | von `run.sh` eingelesen: zwei weitere Starts über `escape-launch` mit dem Backend des Laufs, einer mit getauschtem Projektordner (`--test-swap-work`), einer mit einer Einhängung außerhalb des Plans (`--test-unplanned-mount`); entscheidet `esc-2/swapped_folder` und `esc-2/unplanned_mount` (HUM-437) |
| `esc-4-rules.sh` | ESC-4: die Regel-Tabelle, gegen die Regel-Engine aus HUM-022, für den Body-Cap gegen den laufenden Proxy und seit HUM-114 fünfzehnmal über `humanitl rules test` gegen den Daemon des Laufs; dazu `llm_cli_unreachable`, die einzige Probe der Sammlung für `humanitl llm test` |
| `body_cap.py` | die zwei Anfragen von `rule_body_over_cap`: eine über dem Cap, eine genau auf dem Cap, über den Proxy-Socket |
| `esc-5-filesystem.sh` | ESC-5: acht Fälle gegen die gleichnamigen Integrationstests — fünf zu Dateisystem und Terminal (HUM-042, HUM-043), zwei zur Audit-Kette eines echten Daemons (HUM-050), einer zur Wegwerf-Sandbox: ein Wegwerf-Lauf schreibt nie in das Projekt (HUM-350) |
| `humanitl sandbox run --profile test --tests-dir tests/escape -- …` | der Start jeder Suite: dieselbe Kommandozeile, die der Nutzer aufruft (CONVENTIONS.md 3.11) |

**Ein Fall heißt nie wie ein Agent.** Seit HUM-135 entscheidet der Daemon am
wirksamen Kommando, ob der Agent-Adapter beiträgt, und er entscheidet am
Dateinamen. Ein Skript oder Kommando dieser Sammlung, das `opencode` hieße,
bekäme damit still die fünf Dateien und die `OPENCODE_*`-Variablen des
Adapters — und die Suite maßte eine Sandbox, die sie nicht meint. Die Fälle
heißen deshalb nach dem, was sie prüfen, und `sandbox run` bekommt hier immer
`/bin/sh`.

ESC-1 bis ESC-3 laufen **in** der Sandbox, ESC-4 und ESC-5 auf dem Host: die
Regel-Engine entscheidet, bevor irgendetwas die Maschine verlässt, ein `skip`
braucht ohnehin keine Isolation, und ein Fall, der vom Start der Sandbox
abhängt, verschwindet hinter dem ersten Startfehler. Seit HUM-022 ruft ESC-4 zu
jeder Probe den gleichnamigen Test aus
`daemon/crates/rules/tests/escape_table.rs` auf, der `tests/fixtures/esc4.yaml`
auswertet; die Crate ist rein, also befragt kein Werkzeug die Regel-Datei
unmittelbar. Ohne `cargo` auf der Maschine sind die Fälle ein `skip`, nie ein Grün. Der
achte Fall, `rule_body_over_cap`, misst den Deckel gegen eine live
Allow-Regel (HUM-213). Er läuft deshalb erst hinter `rules reload`, wenn
`esc4.yaml` im Daemon des Laufs gilt, und schreibt `allow_rule=matched` nur,
wenn `humanitl rules test` für genau die Anfrage der Probe
(`POST http://blocked.example/upload`) `allow` mit Exit-Code 0 meldet; ist
`cargo` da, muss zusätzlich der gleichnamige Engine-Test grün sein. Dann
antwortet der laufende Proxy auf einen Body ein Byte über
`limits.hold_body_cap_bytes` trotzdem mit `413` und `reason: body_cap`, während
derselbe Body genau auf dem Cap unter der Regel hinausgeht, erst dann
aufgelöst wird und am NXDOMAIN des aufzeichnenden Nameservers als `502` mit
`reason: upstream_dns` endet. Keine Regel hebt den Deckel auf, solange sie
nicht `stream: true` sagt; der Proxy beachtet dieses Feld noch nicht (HUM-057),
und die Regel der Fixture setzt es nicht. Das DNS-Log von ESC-3 hat `run.sh`
zu diesem Zeitpunkt schon gelesen (`dns_host_cases`), die Auflösung stört es
also nicht. Die Host-Suiten laufen seit HUM-022 vor `stop_escape_daemon`, und
`run.sh` reicht Socket und Cap dieses Laufs in `ESC_PROXY_SOCK` und
`ESC_BODY_CAP` weiter (`limits.hold_body_cap_bytes` steht für den Lauf auf
1024 Bytes, damit die Probe schnell bleibt).

Seit HUM-064 startet jede Suite unter bwrap über `humanitl sandbox run --profile test`, nicht
mehr über das Ad-hoc-Binary `escape-launch` (seit HUM-435 wieder für eine Stufe
vor ihrer Zulassung, siehe „Je Backend"). Darunter liegt derselbe
`BwrapBackend`; der Unterschied ist, dass die Escape-Tests damit genau den
Codepfad prüfen, den später auch der Nutzer nimmt. Das Profil wird beim Namen
genannt und dafür vor dem Lauf nach
`$XDG_CONFIG_HOME/humanitl/profiles/sandbox/` kopiert, der Shim neben das
Binary `humanitl` gelegt: `sandbox run` kennt weder `--profile <pfad>` noch
`--shim`, weil ein geklontes Repository die Politik der Sandbox nicht stellen
darf. Nur `--tests-dir` bleibt ein eigenes Flag; es zieht die Quelle des Binds
nach `/tests/escape` auf das Verzeichnis dieser Skripte.

### Die Helfer in `lib.sh`

| Helfer | Grün, wenn |
|---|---|
| `probe NAME CMD…` | `CMD` **scheitert** — jede Zeile mit `probe` ist ein Exfiltrationsversuch |
| `probe_eperm NAME CMD…` | `CMD` meldet `refused: EPERM`, die Antwort des Filters; `EAFNOSUPPORT` (der Kernel kennt die Familie nicht) ist ein `skip`, jeder andere errno rot |
| `probe_syscall NAME CMD…` | `CMD` meldet `<syscall>: EPERM`, die Antwort des Filters auf `deny_syscalls`; `ENOSYS` und `EINVAL` (der Kernel hat den Syscall gar nicht) sind ein `skip`, jeder andere errno rot, ein Erfolg ein `LEAK` |
| `expect_ok NAME CMD…` | `CMD` gelingt (erlaubte Operationen, etwa `socket(AF_INET)`) |
| `expect_output NAME PAT CMD…` | eine Ausgabezeile matcht `grep -E PAT` |
| `expect_only NAME PAT CMD…` | die Ausgabe ist nicht leer und **jede** Zeile matcht |
| `expect_empty NAME CMD…` | die Ausgabe hat keine nicht-leere Zeile |
| `skip NAME GRUND…` | wird als `skipped` verbucht, nicht als grün |
| `esc_find_sockets ROOT` | keine Probe, sondern die Socket-Liste, die ESC-2 misst; steht in `lib.sh`, damit `selftest.sh` denselben Befehl prüft |

Ein Befehl, den es nicht gibt, endet mit 127 und wird von jedem Helfer als `skip`
verbucht, nie als `pass`. „Das Werkzeug fehlt" darf nie wie „die Sandbox hat
gehalten" aussehen; das ist der billigste Weg zu einer Wand aus falschem Grün.
`selftest.sh` prüft genau diese Zuordnungen, bevor `run.sh` dem Harness eine echte
Sandbox anvertraut.

## Was ESC-1 behauptet

`AF_INET` und `AF_INET6` mit `SOCK_STREAM` müssen **funktionieren** — so erreicht
der Agent den Proxy auf `127.0.0.1:3128` (CONVENTIONS.md 4.10). Ebenso
`socketpair()`: es kennt nur `AF_UNIX`, verbindet zwei Deskriptoren desselben
Prozessbaums und ist kein Egress; Node und Bun brauchen es für die IPC mit
Kindprozessen, und der Filter lässt es unberührt (CONVENTIONS.md 4.11). Die
Garantie lautet nicht „keine Sockets", sondern „kein Weg nach draußen": Das
Netz-Namespace kennt nur `lo`, die Routing-Tabelle ist leer, und jede andere
Familie und jeder andere Typ wird abgewiesen — mit `EPERM`, der Antwort des
Filters (4.10), nicht mit irgendeinem Fehler. `probe_eperm` liest den errno aus
der Ausgabe: `EAFNOSUPPORT` heißt „dieser Kernel hat die Familie gar nicht" und
ist ein `skip`, jeder andere errno ist rot, weil dann nicht der Filter
geantwortet hat. Dasselbe gilt für `io_uring_setup`: `ENOSYS` wäre nur ein
Kernel ohne io_uring, und ein Kernel mit `kernel.io_uring_disabled` antwortet
`EPERM` selbst — der Fall ist dann ein `skip`, kein Grün.

Der Filter vergleicht den Typ als `arg1 & 0xff` (4.10), damit die Flags
`SOCK_NONBLOCK` und `SOCK_CLOEXEC`, die jede Ereignisschleife setzt, auf einem
erlaubten Typ durchgehen und ein verbotener Typ verboten bleibt, gleich welches
Flag mitfährt. Beide Hälften der Regel haben eine Probe:
`socket_inet_stream_flags` und `socket_inet6_stream_flags` müssen
`SOCK_STREAM|SOCK_NONBLOCK|SOCK_CLOEXEC` anlegen können, `socket_inet_dgram_flags`
verlangt `EPERM` für `SOCK_DGRAM|SOCK_CLOEXEC`.

Jeder Name aus `deny_syscalls` (CONVENTIONS.md 3.4) hat eine eigene Probe, die
den Syscall roh wählt (`esc_syscall`, per `ctypes`, Nummern je Architektur für
x86_64, aarch64 und riscv64; andere Maschinen sind ein `skip`, der die
Architektur nennt) und `EPERM` verlangt. `probe_syscall` liest den errno:
`ENOSYS` und `EINVAL` heißen „dieser Kernel hat den Syscall gar nicht"
(kein `CONFIG_KEYS`, kein io_uring) und sind ein `skip`; jeder andere errno ist
rot, weil dann der Kernel geantwortet hat und nicht der Filter — `ENOTSUP` für
einen Deskriptor, der kein Ring ist, `ENOKEY` für einen Schlüssel, den niemand
angelegt hat. Die Argumente sind so gewählt, dass ein Kernel ohne Filter den
Aufruf durchlässt oder mit genau so einem errno antwortet, nie mit `EINVAL`:
`ptrace(PTRACE_TRACEME)`, `process_vm_readv`/`writev` auf den eigenen Prozess,
`keyctl(KEYCTL_GET_KEYRING_ID)` und `add_key` auf den Prozess-Schlüsselring,
`request_key` ohne Callout (ein Callout liefe als `/sbin/request-key` im
Namensraum des Hosts, genau deshalb steht der Syscall auf der Liste),
`io_uring_enter` und `io_uring_register` auf eine eigene Pipe, `pidfd_getfd`
auf Deskriptor 0 des eigenen Prozesses über einen Prozess-Deskriptor aus
`pidfd_open` (HUM-203). Scheitert schon `pidfd_open`, ist `pidfd_getfd` nie
gefragt worden: `probe_pidfd_getfd` wertet das als `skip` (`ENOSYS`) oder als
rot, nie als Verweigerung von `pidfd_getfd`; `pidfd_open_allowed` hält fest,
dass der erste Schritt erlaubt bleibt. Yama mit
`kernel.yama.ptrace_scope` ≥ 2 verweigert `PTRACE_TRACEME` selbst mit `EPERM`,
bevor ein Filter gefragt ist; wie bei `io_uring_disabled` ist der Fall dann ein
`skip`.

### Verweigert heißt auch gemeldet (HUM-138)

Bis HUM-138 war jede Verweigerung still: Der Agent las sein `EPERM`, der Mensch sah eine ruhige
Oberfläche. Seit HUM-138 legt der Filter des Agenten einen verweigerten `socket()`-Aufruf dem
Elternprozess des Shims vor, der ihn zählt und `EPERM` antwortet. `esc-1/refusals_reported`
misst beide Hälften in einer eigenen Sandbox: Jeder der fünf Versuche muss `EPERM` bekommen
(sonst endet der Befehl mit 9), und der Bericht muss genau `3x socket(AF_UNIX, SOCK_STREAM)` und
`2x socket(AF_INET, SOCK_DGRAM)` nennen. Genau, nicht mindestens: Zählte der Bericht die eigenen
Proben des Shims mit (`families`), stünde dort eine höhere Zahl. Ohne `python3` ist der Fall ein
`skip`.

`seccomp_no_own_listener` fragt aus der Sandbox nach einem Filter mit eigenem Zuhörer
(`SECCOMP_FILTER_FLAG_NEW_LISTENER`) und verlangt `EPERM`. `EBUSY` ist rot: Dann hat nur der
lebende Zuhörer des Eltern-Shims den Aufruf abgewiesen, und stirbt der, bekäme der Agent seinen
Zuhörer und könnte seine verweigerten Sockets selbst freigeben.

## Was ESC-2 behauptet

Genau ein Unix-Socket ist in der Sandbox zu finden, und er ist der Proxy. Die
Liste dahinter ist `esc_find_sockets` in `lib.sh`, und zwei Details tragen sie:
`-xtype s` statt `-type s`, weil `bwrap` den Proxy-Socket über eine leere
reguläre Datei auf seinem Root-tmpfs einhängt und `find` bei einem nackten
`-type` dem `d_type` aus `readdir` traut, das für diesen Mountpoint „reguläre
Datei" sagt; und kein `-xdev`, weil `/work`, `/tmp` und `/home/agent` eigene
Mounts sind und ein Socket, den ein Agent in `/work` anlegt, genau das ist, was
gefunden werden muss. `/proc`, `/sys` und `/dev` werden namentlich ausgelassen —
mit einer Ausnahme: `/dev/shm` ist ein beschreibbares tmpfs, der eine Ort unter
`/dev`, an dem ein Agent einen Socket anlegen kann, und wird durchsucht.

Die erste Fassung hatte beides falsch und konnte den Proxy-Socket strukturell nie
finden. Deshalb prüft `selftest.sh` die Probe jetzt gegen einen Socket, dessen
Ort bekannt ist: einen gewöhnlichen in einem temporären Verzeichnis, einen unter
einem eigenen `dev/shm` (neben zweien in `dev` und `dev/pts`, die nicht
auftauchen dürfen) und, wo `bwrap` startet, denselben Socket per Bind-Mount über
ein tmpfs eingehängt, wie `escape-launch` es mit dem Proxy-Socket tut, neben
einem Verzeichnis-Bind auf einem anderen Dateisystem, wie `/work` einer ist.
Alle müssen auftauchen, bevor `run.sh` dem Harness eine Sandbox anvertraut.

Seit HUM-021 startet `run.sh` vor den Suiten einen echten `humanitld` in einem
eigenen XDG-Baum unter `target/escape`. Davor band der Starter dort einen
Platzhalter ein, einen gebundenen Socket, hinter dem niemand antwortet; jede
ESC-3-Probe, die nach der **Antwort** des Proxys fragt, bekam deshalb
„connection refused" statt des Block-Bodys. Proxy-Socket, CA und Token findet
die Kommandozeile selbst über `humanitl_config::Paths`; sie läuft dafür in
genau dem XDG-Baum dieses Daemons, mit `XDG_RUNTIME_DIR=<state>/runtime`, damit
der Socket dort liegt, wo die Mount-Politik ihn erwartet. `start_daemon` in
`tests/e2e/lib.sh` macht für das Demoskript dasselbe.

## Was ESC-3 auf dem Host beweist

ADR-006 sagt: Ein Name wird erst nach der Freigabe aufgelöst. Im Daemon
beweist das `daemon/crates/proxy/tests/dns_after_allow.rs` mit einem
zählenden Mock-Resolver; von außen beweist es seit HUM-115 ESC-3. `run.sh`
startet vor dem Daemon `dns-stub.py`, einen Nameserver auf einem freien
UDP-Port von `127.0.0.1` (nie 53: auf CI-Läufern lauscht dort oft
`systemd-resolved`), und startet den Daemon des Laufs mit
`HUMANITL_RESOLVER__NAMESERVER=127.0.0.1:<port>` und
`HUMANITL_RESOLVER__CACHE_TTL_SECS=0`. Der Daemon löst dann über
`HickoryResolver` auf, der genau diesen Server fragt: kein `/etc/resolv.conf`,
kein `/etc/hosts`, kein zweiter Server, kein eigener Zwischenspeicher. Das
Protokoll des Stubs, `target/escape/dns.log`, ist damit die vollständige Liste
dessen, was der Daemon aufgelöst hat, mit der Zeit jeder Frage.

ESC-3 schickt am Ende zwei Anfragen: eine an `held.esc3.test`, über die
niemand entscheidet, und eine an `allowed.esc3.test`. Neben ESC-3 läuft auf dem
Host ein Beobachter (`dns-proof.sh`), der die Warteschlange über
`humanitl --json flows list 'state:held host:…'` verfolgt. Sieht er den ersten
Flow wartend, kopiert er das Protokoll; sieht er den zweiten, kopiert er es
ebenfalls, merkt sich die Zeit und gibt ihn mit `humanitl --json flows decide
<id> allow` frei, innerhalb der zwei Sekunden Frist dieses Laufs. Der Stub
antwortet `NXDOMAIN`, die freigegebene Anfrage endet deshalb als 502 mit
`reason: upstream_dns`; ein Stub, der `127.0.0.1` lieferte, erzeugte stattdessen
die Ablehnung einer privaten Adresse.

Nach den Suiten in der Sandbox und vor ESC-4 entscheidet `dns-proof.sh` drei
Fälle:

| Fall | Grün, wenn |
|---|---|
| `dns_not_before_decision` | die Kopie, während `held.esc3.test` wartete, keine Frage nach diesem Namen enthält; die Kopie vor der Freigabe keine nach `allowed.esc3.test`; und das ganze Protokoll keine nach einem Namen, den ESC-3 ohne Freigabe geschickt hat |
| `dns_after_allow_once` | das Protokoll genau eine Frage nach `allowed.esc3.test` enthält, ihr Zeitstempel nach der Freigabe liegt, danach keine Zeile mehr folgt und `humanitl --json flows show <id>` `error: upstream_dns` zeigt |
| `meta_no_dns_lookup` | ESC-3 von `humanitl.internal` beantwortet wurde (`meta_status` grün) und das Protokoll keine Frage nach diesem Namen enthält |

Jede Aussage über ein Fehlen steht auf einer Aussage über ein Vorhandensein im
selben Protokoll: Fehlt die Frage nach `allowed.esc3.test`, hat der Daemon den
Stub nie gefragt, und alle drei Fälle sind rot statt grün. Ohne `python3` auf
dem Host gibt es keinen Stub; dann sind die drei Fälle und die zwei Anfragen in
ESC-3 ein `skip`.

## Erwartetes Ergebnis in Sprint 0

Stand nach HUM-203: 136 Fälle, 136 grün, 0 rot, 0 übersprungen, gemessen am
2026-09-23 bei `kernel.yama.ptrace_scope = 0`; HUM-203 bringt zehn Fälle in ESC-1
(`seccomp_parent_mode_2`, `pidfd_open_allowed`, `pidfd_getfd_self`, `pid1_is_shim`,
`pid1_not_dumpable`, `shim_parent_not_dumpable`, `pid1_mem_rdwr`,
`shim_parent_mem_rdwr`, `pid1_pidfd_getfd`, `shim_parent_pidfd_getfd`). Bei
`ptrace_scope` ≥ 1 sind die letzten vier ein `skip`, weil Yama dort selbst
verweigert. Davor, nach HUM-138: 126 Fälle, 126 grün, 0 rot, 0 übersprungen, gemessen am
2026-09-23; HUM-138 bringt `esc-1/refusals_reported` und `esc-1/seccomp_no_own_listener`. Davor, nach HUM-050:
124 Fälle, 124 grün, 0 rot, 0 übersprungen, gemessen am 2026-09-11. HUM-050 macht aus den zwei übersprungenen ESC-5-Fällen
`audit_delete_is_detected` und `audit_truncate_is_detected` grüne. Davor,
nach HUM-115, waren es 124/122/0/2. HUM-115 macht aus den zwei übersprungenen Fällen
`dns_not_before_decision` und `meta_no_dns_lookup` die drei grünen
Host-Fälle `esc-3/dns_not_before_decision`, `esc-3/dns_after_allow_once` und
`esc-3/meta_no_dns_lookup` und bringt die zwei Anfragen
`via_proxy_dns_probe_held` und `via_proxy_dns_allowed_upstream_dns` in ESC-3.
Davor stand hier der Stand nach HUM-114, 121/117/0/4; unmittelbar vor HUM-114
waren es 105/101/0/4, HUM-114 brachte die sechzehn Fälle `rules_cli_01` bis
`rules_cli_15` und `llm_cli_unreachable`. Die Zahl 97/90/0/7 war der Stand
nach HUM-022, also vor HUM-042 und HUM-043. Die Tabelle unten ist der Stand
aus Sprint 0 und nennt zu jeder Probe das Issue, das sie grün gemacht hat; rot
ist keine mehr, übersprungen auch nicht.

`run.sh` legt seine Sockets unter `target/escape/state/runtime` an, und ein
Unix-Socket-Pfad darf höchstens 107 Bytes lang sein. Liegt das Repository
tief, etwa in einem Arbeitsbaum unter `.claude/worktrees/`, startet der Daemon
nicht (`CONFIG_003`, Exit 2); ein kurzer symbolischer Link auf das Repository,
über den `run.sh` aufgerufen wird, genügt.

### Rot, und was sie grün macht

| Suite | Probe | Heute | Grün mit |
|---|---|---|---|
| ESC-1 | `socket_af_unix` | `socket(AF_UNIX)` gelingt | HUM-012 (seccomp: nur `allow_families`, sonst `EPERM`) |
| ESC-1 | `socket_af_netlink` | `socket(AF_NETLINK)` gelingt | HUM-012 |
| ESC-1 | `socket_af_vsock` | `socket(AF_VSOCK)` gelingt (auf einem Kernel ohne vsock: `EAFNOSUPPORT`, also `skip`) | HUM-012 |
| ESC-1 | `socket_inet_dgram` | `SOCK_DGRAM` gelingt | HUM-012 (`allow_types`, `arg1 & 0xff`) |
| ESC-1 | `socket_inet_dgram_flags` | `SOCK_DGRAM\|SOCK_CLOEXEC` gelingt | HUM-012 (die Maske `arg1 & 0xff` streift das Flag ab, der Typ bleibt verboten) |
| ESC-1 | `io_uring_setup` | liefert einen Ring | HUM-012 (`deny_syscalls`, Antwort `EPERM`) |
| ESC-1 | `io_uring_enter` | `ENOTSUP` statt `EPERM`: der Kernel hat den Deskriptor geprüft, kein Filter hat verweigert | HUM-012 (`deny_syscalls`) |
| ESC-1 | `io_uring_register` | `ENOTSUP` statt `EPERM` | HUM-012 (`deny_syscalls`) |
| ESC-1 | `ptrace_traceme` | `ptrace(PTRACE_TRACEME)` gelingt (bei `kernel.yama.ptrace_scope` ≥ 2: `EPERM` vom Kernel, also `skip`) | HUM-012 (`deny_syscalls`) |
| ESC-1 | `process_vm_readv` | liest 16 Byte aus dem eigenen Prozess | HUM-012 (`deny_syscalls`) |
| ESC-1 | `process_vm_writev` | schreibt 16 Byte in den eigenen Prozess | HUM-012 (`deny_syscalls`) |
| ESC-1 | `keyctl_get_keyring_id` | liefert die Seriennummer des Prozess-Schlüsselrings | HUM-012 (`deny_syscalls`) |
| ESC-1 | `add_key` | legt einen `user`-Schlüssel an | HUM-012 (`deny_syscalls`) |
| ESC-1 | `request_key` | `ENOKEY` statt `EPERM`: der Kernel hat gesucht, kein Filter hat verweigert | HUM-012 (`deny_syscalls`) |
| ESC-1 | `x32_syscall_eperm` | `ENOSYS` statt `EPERM` | HUM-012 (BPF-Präludium, CONVENTIONS 4.10) |
| ESC-1 | `seccomp_mode_2` | `Seccomp: 0` | HUM-012 |
| ESC-1 | `seccomp_every_process` | jeder Prozess `Seccomp: 0`; seit HUM-203 ohne Ausnahme, auch PID 1 | HUM-012, HUM-203 |
| ESC-1 | `seccomp_parent_mode_2` | `/proc/1/status` zeigt `Seccomp: 0`: PID 1 war das Init von `bwrap` | HUM-203 (`--as-pid-1`, der Shim ist PID 1) |
| ESC-1 | `pidfd_getfd_self` | kopiert Deskriptor 0 des eigenen Prozesses | HUM-203 (`pidfd_getfd` im Boden) |
| ESC-1 | `pid1_is_shim` | `/proc/1/comm` ist `bwrap` | HUM-203 |
| ESC-1 | `pid1_not_dumpable`, `shim_parent_not_dumpable` | `/proc/1/mem` gehört der UID des Agenten, der Prozess ist dumpable | HUM-203 (`PR_SET_DUMPABLE 0` als erster Schritt in `parent()`) |
| ESC-1 | `pid1_mem_rdwr`, `shim_parent_mem_rdwr` | `open("/proc/1/mem", O_RDWR)` gelingt bei `kernel.yama.ptrace_scope=0` (bei ≥ 1 verweigert Yama selbst, also `skip`) | HUM-203 |
| ESC-1 | `pid1_pidfd_getfd`, `shim_parent_pidfd_getfd` | `pidfd_getfd` auf PID 1 liefert dessen Deskriptor (bei `ptrace_scope` ≥ 1: `skip`) | HUM-203 |
| ESC-2 | `exactly_one_socket` | null Sockets | HUM-011/013 (Daemon reicht den Proxy-Socket durch) |
| ESC-2 | `socket_is_proxy` | kein Socket zu benennen | HUM-011/013 |
| ESC-2 | `no_marker_leak` | `/proc/1/environ` trägt die Host-Umgebung | HUM-011 (siehe unten) |
| ESC-3 | `via_proxy_held` | keine Antwort, kein Proxy | HUM-013/015 plus HUM-021 (der Daemon hinter dem Socket) |
| ESC-3 | `via_proxy_private_held` | keine Antwort | wie oben; als `PrivateAddress` statt als Zeitüberschreitung mit HUM-024 |
| ESC-3 | `via_proxy_metadata_held` | keine Antwort | wie oben |
| ESC-3 | `via_proxy_idn_held` | keine Antwort | wie oben |
| ESC-3 | `via_proxy_reason_line` | keine Antwort | wie oben (Body-Format CONVENTIONS 3.5) |
| ESC-3 | `host_mismatch_blocked` | grün seit HUM-021: ein `Host`, der dem CONNECT-Ziel oder der Anfragezeile widerspricht, wird ohne Rückfrage als `authority_mismatch` geblockt | HUM-023 ergänzt die SNI-Prüfung |

### Befund für HUM-011: `/proc/1/environ`

`no_marker_leak` ist nicht aus demselben Grund rot wie die übrigen. `run.sh` setzt
`HUMANITL_ESCAPE_MARKER` in seine eigene Umgebung und sucht in der Sandbox nach
dem **Namen** der Variablen (nie nach ihrem Wert — den mitzugeben hieße, genau das
zu pflanzen, wonach die Probe sucht). Gefunden wird er, und zwar in
`/proc/1/environ`:

```
esc-2 no_marker_leak fail LEAK: the attempt succeeded: /proc/1/environ
```

`--clearenv` räumt die Umgebung des Kindes auf, nicht die von `bwrap` selbst.
`bwrap` bleibt mit `--unshare-pid` als PID 1 zurück, läuft unter derselben UID,
und `/proc/1/environ` ist damit aus der Sandbox lesbar. Wer im Agenten sitzt,
liest die vollständige Umgebung des Hosts: `XDG_RUNTIME_DIR`, `SSH_AUTH_SOCK`,
Token, alles.

HUM-011 muss `bwrap` deshalb mit einer bereinigten Umgebung starten (nur das, was
für den Start nötig ist) oder `--as-pid-1` verwenden. Bis dahin bleibt die Probe
rot — sie ist der Grund, warum sie existiert.

Nachtrag HUM-203: HUM-011 hat die bereinigte Umgebung gebaut, seit HUM-203 kommt
`--as-pid-1` dazu. PID 1 ist jetzt der Shim-Elternprozess; er trägt den Filter
der Brücke und ist nicht dumpable, `/proc/1/environ` und `/proc/1/mem` gehören
damit root (vom Host aus gesehen; in der Sandbox die Überlauf-UID 65534) und sind für den Agenten weder lesbar noch beschreibbar. Das Init von
`bwrap` war ungefiltert und dumpable, bei `ptrace_scope=0` öffnete der Agent
`/proc/1/mem` zum Schreiben (Sicherheitsdurchlauf vom 2026-09-23, Befund M1).
Die Proben `pid1_*` und `shim_parent_*` in ESC-1 halten das fest; bei
`ptrace_scope` ≥ 1 verweigert schon Yama, dort trägt `pid1_not_dumpable`.
Gelesen wird der Besitzer von `/proc/1/mem`, nicht der des Verzeichnisses
`/proc/1`: Das Verzeichnis behält auch bei einem nicht-dumpable Prozess dessen
UID (`task_dump_owner` im Kernel), nur die Dateien darin gehen an root. Die Probe
urteilt nur über zwei echte UIDs: Fehlt `stat` oder `id`, ist sie ein `skip`,
scheitert eines davon, ist sie rot, nie grün.

### Übersprungen, und was sie einlöst

Derzeit keiner. Die zwei ESC-5-Fälle `audit_delete_is_detected` und
`audit_truncate_is_detected`, die hier bis HUM-050 auf die Hash-Kette des
Audit-Logs warteten, prüfen jetzt die Kette eines echten Daemons und sind grün.

Die zwei Fälle aus ESC-3, die hier bis HUM-115 standen
(`dns_not_before_decision`, `meta_no_dns_lookup`), sind jetzt Host-Fälle und
grün, siehe „Was ESC-3 auf dem Host beweist". Ohne `python3` auf dem Host
werden sie wieder übersprungen, zusammen mit `dns_after_allow_once` und den
zwei Anfragen in ESC-3.

### Grün, und warum jetzt schon

Die grünen Fälle halten heute, weil sie nicht am Shim hängen, sondern am Profil
und am Namespace: nur `lo`, leere IPv4-Routing-Tabelle, keine Capabilities
(`--cap-drop ALL`), `NoNewPrivs: 1`, kein X11-, Wayland-, D-Bus- oder
Docker-Socket, kein Host-`/home`, kein `$XDG_RUNTIME_DIR`, kein Host-`/sys`,
`hostname` gleich `sandbox`, `/dev/shm` als tmpfs, `socketpair()` funktioniert
(erlaubt, siehe oben), `SOCK_STREAM` mit `SOCK_NONBLOCK|SOCK_CLOEXEC` lässt sich
auf `AF_INET` und `AF_INET6` anlegen (und muss es nach HUM-012 weiterhin, siehe
oben), und die maskierten Pfade
(`/work/.envrc`, `/work/.git/config`, `/work/.git/hooks`, `/work/.vscode`,
`/work/.idea`) geben den Kanarienvogel nicht heraus, den `run.sh` vorher in die
Host-Kopie schreibt. Auch der direkte Egress ist bereits dicht: `curl --noproxy`,
`getent hosts`, `bash -c 'exec 3<>/dev/tcp/…'` und UDP nach draußen scheitern an
`ENETUNREACH`, lange bevor seccomp geschrieben ist. Eine rote Zeile in diesem
Block wäre kein „noch nicht", sondern ein Fehler.

Drei Proben sind grün, ohne trivial zu sein, und die Begründung steht jeweils
im Skript: `socket_inet_raw` fragt mit `IPPROTO_ICMP` nach einem echten
Raw-Socket und scheitert heute an `CAP_NET_RAW` (`--cap-drop ALL`), ab HUM-012
zusätzlich an der Typ-Maske des Filters; `socket_af_packet` ebenso. Beide
antworten heute wie später `EPERM`, was `probe_eperm` verlangt. Mit dem
Protokoll 0 hätte der Kernel `EPROTONOSUPPORT` geantwortet, bevor irgendeine
Sandbox gefragt ist, und die Probe wäre für immer grün gewesen. `dev_tcp` läuft unter `bash`, weil `/bin/sh`
(dash) `/dev/tcp` gar nicht kennt und die Probe darunter grün wäre, weil die
Shell die Umleitung nicht versteht; ohne `bash` ist der Fall ein `skip`.

## Kein root

Das Harness braucht keine erhöhten Rechte. Der einzige Punkt, an dem `run.sh` es
versucht, ist die AppArmor-Einschränkung unprivilegierter User-Namespaces auf
Ubuntu 24.04, und zwar mit `sudo -n`: gibt es kein passwortloses `sudo`, sagt der
Lauf das und macht weiter. Wo `python3` nötig ist statt eines Shell-Builtins, wird
`python3` verwendet — es liegt in `/usr`, und `/usr` ist nur lesbar eingehängt.

## Eine Probe hinzufügen

1. Zeile in die passende `esc-N-*.sh` schreiben. `probe` für einen Versuch, der
   scheitern muss, `expect_*` für eine Beobachtung, die eintreten muss.
2. `./tests/escape/run.sh` laufen lassen und den Beleg in `results.txt` lesen. Eine
   Probe, deren Beleg nichts sagt, taugt nicht: der Text landet in `escape.xml`
   und ist das, was jemand in sechs Monaten in CI vor sich hat.
3. Ist sie in Sprint 0 rot, hier in die Tabelle eintragen, mit dem Issue, das sie
   grün macht.
