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
| `esc-3-egress.sh` | ESC-3: kein Egress ohne Proxy, über den Proxy landet alles in der Warteschlange |
| `esc-4-rules.sh` | ESC-4: die Regel-Tabelle, gegen die Regel-Engine aus HUM-022 und, für den Body-Cap, gegen den laufenden Proxy |
| `body_cap.py` | die zwei Anfragen von `rule_body_over_cap`: eine über dem Cap, eine genau auf dem Cap, über den Proxy-Socket |
| `esc-5-filesystem.sh` | ESC-5: Platzhalter, alle Fälle `skipped` (HUM-043/050/029) |
| `humanitl sandbox run --profile test --tests-dir tests/escape -- …` | der Start jeder Suite: dieselbe Kommandozeile, die der Nutzer aufruft (CONVENTIONS.md 3.11) |

ESC-1 bis ESC-3 laufen **in** der Sandbox, ESC-4 und ESC-5 auf dem Host: die
Regel-Engine entscheidet, bevor irgendetwas die Maschine verlässt, ein `skip`
braucht ohnehin keine Isolation, und ein Fall, der vom Start der Sandbox
abhängt, verschwindet hinter dem ersten Startfehler. Seit HUM-022 ruft ESC-4 zu
jeder Probe den gleichnamigen Test aus
`daemon/crates/rules/tests/escape_table.rs` auf, der `tests/fixtures/esc4.yaml`
auswertet; die Crate ist rein, also gibt es bis `humanitl rules test URL`
(HUM-065) kein Werkzeug, das eine Regel-Datei von der Kommandozeile aus
befragt. Ohne `cargo` auf der Maschine sind die Fälle ein `skip`, nie ein Grün. Der
achte Fall, `rule_body_over_cap`, hat zwei Hälften: die Engine erlaubt den Host
per Regel, und der laufende Proxy antwortet auf einen Body über
`limits.hold_body_cap_bytes` trotzdem mit `413` und `reason: body_cap`, während
ein Body genau auf dem Cap gehalten wird und in die Zeitüberschreibung läuft.
Deshalb laufen die Host-Suiten seit HUM-022 vor `stop_escape_daemon`, und
`run.sh` reicht Socket und Cap dieses Laufs in `ESC_PROXY_SOCK` und
`ESC_BODY_CAP` weiter (`limits.hold_body_cap_bytes` steht für den Lauf auf
1024 Bytes, damit die Probe schnell bleibt).

Seit HUM-064 startet jede Suite über `humanitl sandbox run --profile test`, nicht
mehr über das Ad-hoc-Binary `escape-launch`. Darunter liegt derselbe
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
`io_uring_enter` und `io_uring_register` auf eine eigene Pipe. Yama mit
`kernel.yama.ptrace_scope` ≥ 2 verweigert `PTRACE_TRACEME` selbst mit `EPERM`,
bevor ein Filter gefragt ist; wie bei `io_uring_disabled` ist der Fall dann ein
`skip`.

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

## Erwartetes Ergebnis in Sprint 0

Stand nach HUM-022: 97 Fälle, 90 grün, 0 rot, 7 übersprungen (nach HUM-064
waren es 82 grün und 15 übersprungen; ESC-4 hat alle acht Fälle eingelöst). Die Tabelle
unten ist der Stand aus Sprint 0 und nennt zu jeder Probe das Issue, das sie
grün gemacht hat; rot ist keine mehr. Übersprungen bleibt, was auf ein Issue
späterer Sprints wartet, darunter `dns_not_before_decision`: dass die Sandbox
keinen Namen auflöst, zählt erst der Resolver aus HUM-024.

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
| ESC-1 | `seccomp_every_process` | jeder Prozess `Seccomp: 0` (PID 1, also `bwrap`, ist die per PID benannte Ausnahme, nicht per `comm`: den Namen kann sich jeder Prozess selbst geben) | HUM-012 |
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

### Übersprungen, und was sie einlöst

| Suite | Fälle | Wartet auf |
|---|---|---|
| ESC-3 | `dns_not_before_decision` | HUM-024 (Auflösung erst nach der Freigabe, ADR-006) plus ein host-seitiger DNS-Beobachter in `run.sh` |
| ESC-5 | 6 Fälle Dateisystem, Terminal, Audit | HUM-043 (Symlinks, Maskierung), HUM-050 (OSC 52/8), HUM-029 (Hash-Kette) |

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
