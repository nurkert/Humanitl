---
source: THREAT-MODEL.md
source-sha256: 06b8f324ee21422f0f7c6fb17102a01a116253e19e9128c60ebd041050954024
lang: de
---
# Bedrohungsmodell

Applies to: Humanitl 0.1

Dieses Dokument sagt, gegen wen die Mechanismen aus [`SECURITY.md`](SECURITY.md) wirken und wo
nicht. Wer keinen Sicherheitshintergrund hat, soll nach den ersten drei Abschnitten wissen, was
Humanitl verhindert und was nicht. Wer prüft, soll jede Behauptung nachprüfen können: Für jeden
Kanal sind der Mechanismus, die Datei, die ihn umsetzt, und der Test, der ihn beobachtet, genannt.

Humanitl schützt zwei Dinge. **Vertraulichkeit von Projektdaten und Zugangsdaten:** Nichts aus dem
Projektverzeichnis, der Umgebung des Agenten oder seinem Speicher verlässt den Rechner über das
Netzwerk, es sei denn, eine Regel oder ein Mensch hat genau diese Anfrage freigegeben; ohne
passende Regel sieht ein Mensch sie zuerst, mit Methode, Ziel, Headern, Parametern und dem
vollständigen Body der Anfrage. **Integrität der Entscheidungsgeschichte:** Was wann und warum
entschieden wurde, kann niemand, dem der HMAC-Schlüssel der Audit-Kette fehlt, später unbemerkt
umschreiben. Ein Prozess, der als derselbe Nutzer läuft, kann diesen Schlüssel lesen und Kette und
Anker neu bauen; gegen ihn beweist die Kette nichts (K-14).
**Verfügbarkeit des Agenten ist ausdrücklich kein Ziel:** Fällt ein Mechanismus der Isolation oder
der Entscheidung aus, hält der Agent an, und eine Zeitüberschreitung endet in `block`, nie in
`allow`. Ein Ausfall hält den Agenten gar nicht an: Ein Ausfall der Aufzeichnung stoppt die
Aufzeichnung, nicht den Verkehr (HUM-250, K-16). Und eine rote Isolationsprüfung hält den Agenten
zwar an, aber erst nachdem er gestartet ist, nicht vorher (K-15).

Zwei Ausnahmen sind erklärt und sichtbar: die Durchleitung zum Sprachmodell (durchgereicht,
vollständig aufgezeichnet, gescannt, nicht angehalten) und das Projektverzeichnis `/work`. Eine
dritte schreibt der Mensch in seine eigenen Regeln: Eine Regel mit `stream: true` lässt eine
Anfrage, deren Body über `limits.hold_body_cap_bytes` liegt, durchgereicht hinaus, entschieden nach
ihrem Kopf, den Body ungescannt; sie greift nie, solange `hold.hard_block_checksum_secrets` an ist.

## Schutzgüter

| Schutzgut | Wo es liegt | Warum es zählt | Geschützt durch |
|---|---|---|---|
| Projektdaten | das Projektverzeichnis, eingehängt unter `/work` | Code und Daten von Kunden unter Verschwiegenheits- und DSGVO-Pflichten | die drei Garantien; kein Egress außer freigegebenen Anfragen und dem erklärten LLM-Host |
| Zugangsdaten | die Umgebung des Rechners, `~/.ssh`, `~/.netrc`, Schlüsselbunde, Tokens in anderen Prozessen | ein gestohlenes Token öffnet Systeme weit über das Projekt hinaus | PID-Namespace, Mount-Erlaubnisliste, saubere Umgebung (`--clearenv`), der LLM-Schlüssel nur als Datei in der Sandbox |
| LLM-Prompts | unterwegs zum LLM-Host, in der Aufzeichnung | sie tragen jeden Code, den der Agent dem Modell zeigt | erklärter Seitenkanal; vollständige Aufzeichnung und Scan auf Funde; der Nutzer wählt den Host |
| Audit-Log | `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`, Anker in `humanitl.db` | Beleg dafür, was ein Agent tatsächlich hinausgeschickt hat | Hash-Kette mit HMAC, Anker in Datei und Datenbank |
| CA-Schlüssel | `$XDG_DATA_HOME/humanitl/ca/ca.key`, `0600` | wer ihn hat, kann TLS für jede Verbindung fälschen, die der Humanitl-CA vertraut | nie in der Sandbox, nie im Vertrauensspeicher des Rechners; nur innerhalb der Sandbox vertrauenswürdig |

## Angreifer

In jedem Fall ist der Angreifer **Code, der als der Agent läuft, mit unterwanderter Absicht**. Er
hat die Rechte des Agenten: Er kann in `/work` lesen und schreiben, Prozesse starten und mit dem
Proxy sprechen. Er hat nicht die Rechte des Kernels und nicht die des Rechners.

| ID | Angreifer | Wie er hineinkommt | Was er kann | Was er nicht kann |
|---|---|---|---|---|
| **a** | **Prompt-Injection** über geladene Inhalte | eine README, ein Issue-Text, eine Webseite, eine Fehlermeldung eines Paketservers enthält Anweisungen, die das Modell als Aufgabe liest | die Werkzeugaufrufe des Agenten lenken: Dateien lesen, Anfragen formulieren, Text an das Modell schicken | eigene Binaries mitbringen; er arbeitet mit den Werkzeugen, die der Agent hat |
| **b** | **Bösartige Abhängigkeit** | ein Paket, das der Agent installiert oder ausführt (`npm install`, `pip install`, ein Build-Skript) | beliebiger Code als Agent-Nutzer in der Sandbox: Syscalls, eigene Netzwerkversuche, Lesen jeder sichtbaren Datei | alles, was der Kernel verweigert; keine neuen Mounts, keine Capabilities |
| **c** | **Kompromittiertes Modell** | das lokale Modell selbst ist manipuliert (vergiftete Gewichte, ein untergeschobener Endpunkt) | alles aus (a), und es sieht jeden Prompt, womöglich den ganzen Quelltext, den der Agent ihm zeigt | irgendetwas an der Sandbox ändern; es kann nur den Agenten lenken |
| **d** | **Lokaler Leser der Aufzeichnungen** | ein anderer Prozess oder Mensch mit Dateizugriff auf `$XDG_DATA_HOME/humanitl` | Aufzeichnungen lesen, Einträge löschen oder ändern, die Datei abschneiden | die Kette ohne den HMAC-Schlüssel unbemerkt neu bauen |

(a) und (b) unterscheiden sich stark in ihrer Macht: (a) muss über den Agenten arbeiten, (b) führt
Code aus. Jede Aussage hier gilt für (b), sofern sie nichts anderes sagt, und damit auch für (a).
(c) ist der unangenehmste, weil der Modellkanal offen ist; Humanitl reduziert ihn auf „der Nutzer
hat entschieden, diesem Host zu vertrauen" und macht diese Entscheidung sichtbar (K-02).

## Vertrauensgrenzen

```
 +--------------------------- host, one user account -----------------------------+
 |                                                                                 |
 |  +------------------------+   gRPC over UDS    +------------------------------+ |
 |  | humanitl app / CLI     | <----------------> | humanitld (systemd --user)   | |
 |  | shows, decides         |   daemon.sock 0600 | proxy, rules, recorder,      | |
 |  | (trusted for the       |   + token          | audit, sandbox control       | |
 |  |  decision, not for     |                    | CA key, audit key            | |
 |  |  isolation)            |                    +---------+---------+----------+ |
 |  +------------------------+                       (B2)    |         |  (B3)      |
 |                                       proxy.sock (file)   |         |            |
 |  - - - - - - - - - - - - - - - - - - - - - - - - - - - - -|- - - - -|- - - - - - |
 |  (B1) sandbox: bwrap, own net/pid/ipc/uts/user/cgroup ns  |         |            |
 |  +--------------------------------------------------------v--+      |            |
 |  | shim (PID 1): bridge 127.0.0.1:3128 -> proxy.sock          |      |            |
 |  | agent: seccomp filter, no interface but lo                 |      |            |
 |  | /work (project, ro|rw) <---- shared with the host --------------------- (B4)   |
 |  +------------------------------------------------------------+      |            |
 +----------------------------------------------------------------------|------------+
                                                                        |
                     LLM host (LAN, passthrough rule, recorded)  <------+------>  internet
                                                                   (only after approval)
```

- **B1, Sandbox zum Rechner.** Die Grenze des Kernels: Namespaces, seccomp, keine Capabilities.
  Alles darin ist vom Angreifer beeinflusst, auch die eigenen Erlaubnisabfragen des Agenten; sie
  sind keine Grenze, und Humanitl schaltet sie für Abrufe und Befehle ab
  ([`SECURITY.md`](SECURITY.md#the-agents-own-permission-prompts-hum-308)).
- **B2, die eine Tür.** Die Socket-Datei des Proxys. Jedes Byte, das hindurchgeht, parst der
  Daemon; deshalb ist der Daemon speichersicheres Rust und von systemd gehärtet.
- **B3, Daemon zum Netzwerk.** Erst nachdem eine Regel oder ein Mensch die Anfrage erlaubt hat,
  einmal nach der Entscheidung aufgelöst, festgehalten auf die gefundene Adresse.
- **B4, `/work`.** Absichtlich geteilt. Was der Agent dort schreibt, können die Werkzeuge des
  Rechners später ausführen oder ein Mensch hinaustragen.
- **Das Fenster und die Kommandozeile** sitzen auf der Seite des Rechners. Sie können die Sandbox
  nicht schwächen, aber sie zeigen dem Menschen, worüber er entscheidet.
- **Der Daemon und der Dienstverwalter des Nutzers (HUM-434).** Unter seiner gehärteten
  Nutzer-Unit läuft der Daemon in einem eigenen Nutzer-Namensraum, in dem rootless Podman seine
  Kennungen nicht abbilden kann. Statt diese Härtung zu lockern, startet der Daemon jeden Aufruf
  von Podman als vorübergehende Unit `humanitl-*` des Dienstverwalters des Nutzers
  (`systemd-run --user`), außerhalb seiner eigenen Unit. Ein übernommener Daemon gewinnt damit
  **„eine vorübergehende Unit des Nutzers starten"**: jedes Programm als derselbe Nutzer, ohne
  die Einschränkungen von `humanitld.service`. Das Konto konnte das vorher schon, der gehärtete
  Daemon nicht. Der Code baut nur Unit-Namen mit dem Präfix `humanitl-` und eine feste Zeile
  (`--pipe --wait --collect`, `KillMode=mixed`, das Programm hinter `env -i`); das schützt vor
  Fehlern, nicht vor einem übernommenen Daemon. Unter bubblewrap wird nichts davon benutzt. Die
  Deskriptoren der Sandbox erreichen den Shim über einen Socket der Sitzung, der genau eine
  Verbindung derselben Kennung mit einem Einmal-Zeichen annimmt und danach jede andere ablehnt;
  gebunden wird nur sein Verzeichnis, und der Daemon entfernt den Socket vor dem Senden, sodass
  der Agent dort nichts findet. Das Zeichen steht in der Befehlszeile der Unit
  (`systemctl --user show`, D-Bus); ein Prozess derselben Kennung kann es lesen und dem Shim
  zuvorkommen. Das endet in einer gescheiterten Sitzung, nie darin, dass die Deskriptoren eine
  andere Kennung erreichen.

## Angriffsfläche

Die zwölf Kanäle aus dem Sicherheits-Review vom 2026-09-02. Die Schwere ist die Auswirkung,
**falls** der Kanal genutzt wird, nicht seine Wahrscheinlichkeit. Der Status ist `mitigated` (ein
Mechanismus in 0.1 schließt oder begrenzt ihn), `declared` (absichtlich offen, in der Anwendung
gezeigt, nicht geschlossen) oder `open` (eine bekannte Lücke, für die es noch keinen Mechanismus
gibt).

| ID | Kanal | Angreifer | Schwere | Abhilfe in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-01 | Projektverzeichnis `/work` | a, b, c | hoch | nur ein Unterpfad; Verzeichnisse für Hooks, CI und Editoren sowie Secret-Dateien verdeckt; Zusammenfassung der Session mit berührten Dateien, nach außen zeigenden Symlinks und einem Secret-Scan; der eingehängte Ordner ist der geprüfte (unter bubblewrap über den Deskriptor gebunden, unter jedem Backend Gerät und Inode vom Shim geprüft, `SANDBOX_042`) | declared | ESC-5, ESC-2 (`swapped_folder`) |
| K-02 | LLM-Durchleitung | a, b, c | hoch | genaue Regel auf Host, Port, Schema, Methoden und genaue Inferenz-Endpunkte; vollständige Aufzeichnung; Funde warnen | declared | - |
| K-03 | Socket-Verzeichnis des Proxys | b | mittel | die Socket-**Datei** wird eingebunden, nie ihr Verzeichnis | mitigated | ESC-2 |
| K-04 | von seccomp erlaubte TCP-Sockets | b | niedrig | leerer Netzwerk-Namespace und leere Routing-Tabelle; Elternprozess gefiltert und nicht dumpbar | mitigated | ESC-1 |
| K-05 | Unix-Sockets des Rechners (X11, Wayland, D-Bus, Docker) | b | kritisch | Mount-Erlaubnisliste, Sperrliste im Argument-Builder (`SANDBOX_006`), die Mounttabelle in der Sandbox gleicht dem Plan (`SANDBOX_049`), `AF_UNIX` verweigert | mitigated | ESC-2 |
| K-06 | PID- und IPC-Namespace | b | hoch | eigene PID-, IPC- und UTS-Namespaces, eigenes `/proc`, `--new-session`, Shim als nicht dumpbarer PID 1 | mitigated | ESC-1, ESC-2 |
| K-07 | `/proc` und `/sys` | b | niedrig | das eigene `procfs` des Namespace, `/sys` nicht eingebunden | mitigated | ESC-2 |
| K-08 | Symlinks aus `/work` hinaus | b | mittel | innerhalb des Mount-Namespace aufgelöst; in der Zusammenfassung der Session markiert (`SANDBOX_022`) | mitigated | ESC-5 |
| K-09 | Terminal-Escapes (OSC 52, OSC 8, Titel) | a, b, c | mittel | Filter mit Erlaubnisliste im Daemon vor jedem Client; kein OSC-Handler im Widget | mitigated | ESC-5 |
| K-10 | DNS und der Zeitpunkt der Auflösung | a, b, c | hoch | Auflösung erst nach der Freigabe (ADR-006), festgehaltene Adresse | mitigated | ESC-3 |
| K-11 | Metadaten von Warteschlange und Regeln | a, b, c | niedrig | `humanitl.internal` ist nur lesbar, `/ask` legt nie eine Regel an; das Orakel der Richtlinie bleibt | declared | ESC-4 |
| K-12 | Paket-Caches | b | mittel | Caches je Projekt, nicht je Vertrauensstufe; keine, solange keine konfiguriert sind | declared | - |

Fünf weitere Kanäle vervollständigen das Modell:

| ID | Kanal | Angreifer | Schwere | Abhilfe in 0.1 | Status | Test |
|---|---|---|---|---|---|---|
| K-13 | direkter Egress ins Netzwerk | a, b, c | kritisch | Netzwerk-Namespace, seccomp, keine Capabilities | mitigated | ESC-1, ESC-3 |
| K-14 | Audit-Datei auf dem Rechner | d | hoch | Hash-Kette mit HMAC, Anker in Datei und SQLite; begrenzt nur gegen einen Angreifer ohne den HMAC-Schlüssel: nicht gegen einen Prozess desselben Nutzers, der den Schlüssel hat, nicht gegen einen Schnitt hinter dem letzten Anker | declared | ESC-5 |
| K-15 | das Zeitfenster zwischen `exec` und `SIGKILL` | b | hoch | Bericht sofort gelesen, `SIGKILL` ohne Schonfrist | open | - |
| K-16 | Ausfall der Aufzeichnung | a, b, c | mittel | Ausfall an jeden Client einmal je Ursache gemeldet, Karten als „Wird nicht aufgezeichnet" markiert, erste Freigabe pausiert | declared | Health-Tests in `daemon/crates/recorder`, Golden-Test `states_recorder_failure` |
| K-18 | die Tür für weitere Shells (`humanitl sandbox exec`) | b | hoch | unbenanntes Socket-Paar, das nur der Daemon und PID 1 halten; der Shim forkt die Shell von innen mit dem Filter des Agenten obendrauf, belegt durch ein verweigertes `AF_UNIX` und ein verweigertes `AF_VSOCK`; höchstens vier; stirbt mit der Sandbox; im Strom-Modus (K-29) ist die Anfrage ein Rahmen auf dem Steuerkanal, und der Shim öffnet das Pseudo-Terminal der Shell selbst, unter denselben Grenzen | mitigated | `daemon/crates/ipc/tests/terminal.rs` (HUM-299), `daemon/crates/ipc/tests/terminal_stream.rs` (HUM-443) |
| K-24 | die Wegwerf-Sandbox: die obere Schicht und der Weg zurück ins Projekt (`humanitl sandbox apply`) | b | hoch | Projektordner nur lesbar als untere Schicht eines Overlays, obere Schicht im RAM unter dem Laufzeitverzeichnis des Daemons mit Größengrenze, `mounts.extra_rw` nur lesbar; übernommen nur auf Wahl eines Menschen, geprüft vor dem ersten Schreiben, nie durch einen oder als einen Symlink aus dem Projekt hinaus, kein Gerät, kein FIFO, kein Socket, kein setuid-Bit, kein Überschreiben einer Datei, die der Mensch seit dem Start geändert hat, außer diese Änderung ist einzeln bestätigt (`ApplyChanges.overwrite_ids` aus der App, HUM-351) oder jede ausgewählte Änderung ist es (`--overwrite`); ein Backend mit Container-Engine lehnt `disposable` ab (`SANDBOX_030`), weil sich seine untere Schicht nicht an den geprüften Ordner binden lässt, und `auto` läuft dort direkt (HUM-438) | mitigated | `daemon/crates/sandbox/src/disposable/tests.rs`, `daemon/crates/sandbox/tests/launcher.rs`, `daemon/crates/ipc/tests/sandbox_start.rs` (HUM-350); `daemon/crates/sandbox/src/podman/tests.rs`, `daemon/crates/sandbox/tests/escape_worktree.rs` (HUM-438) |
| K-27 | die Container-Engine unter Podman (Podman, `conmon`, `crun`, der Container-Speicher des Nutzers, `containers.conf`) | b, d | hoch | in der gemeinsamen Stufe (`crun`) erst zugelassen, als die Escape-Suite gegen sie auf `main` grün war (HUM-448), die Kernel-Stufe (Backend `qemu`, ADR-0021) bis HUM-449 mit `SANDBOX_048` abgelehnt, `auto` nie Podman; jede Konfigurationsquelle, die der Daemon nicht schreibt, abgeschnitten; feste Flags ohne Schlüssel; die Umgebung nur über den versiegelten memfd; beendet über die Engine und die Lebensleine; Reste beim Start des Daemons entfernt (`SANDBOX_041`) | mitigated | ESC-1, ESC-2, ESC-5 (`escape-tests-podman`, HUM-435); `daemon/crates/sandbox/src/podman/` (HUM-433) |
| K-28 | das Abbild als Wurzel der Sandbox unter Podman und das Ziehen (Herkunft, Digest, das Netz beim Ziehen) | b, d | hoch | mit Digest festgelegt, vor jedem Start geprüft, von keinem Start gezogen (`SANDBOX_043`); gezogen nur auf Wunsch, von der Engine direkt und nicht durch den Proxy, bei der Registry, die der Name nennt, und vorher gesagt; Zugangsdaten zur Registry nur aus der eigenen Datei des Adapters, der Engine als private Kopie gereicht, ein Credential-Helfer darin oder in einer Ergänzung zur `registries.conf` abgelehnt (HUM-456); Projekt, Tür, CA, Shim und Dateien der Sitzung vom Host, nie aus dem Abbild; Nutzer, Einstiegspunkt, Volumes und Gesundheitsprüfung des Abbilds aufgehoben, ein Abbild mit `HUMANITL_*` abgelehnt | open | ESC-2 `ca_is_the_hosts` (`HUMANITL_ESCAPE_IMAGE`); `daemon/crates/sandbox/src/podman/image.rs` (HUM-439) |
| K-29 | der Steuerkanal der Terminals (der Strom-Modus: die Kernel-Stufe immer, bwrap, wenn ein Aufrufer ihn verlangt) | b | hoch | ein Strom-Socket, den nur der Daemon und PID 1 halten, geöffnet vor dem Filter des Agenten, der `AF_VSOCK` und `AF_UNIX` verweigert; im Gast öffnet ihn der Host als zweite Verbindung am einen lauschenden Port des Shims, der Gast öffnet keine; nur Terminal-Bytes und Steuerung, nie ein Byte an den Proxy oder von ihm; Rahmen von höchstens 16 KiB, übergroße Rahmen, unbekannte Arten und unbekannte Kennungen verwerfen beide Seiten, ohne den Strom zu beenden; sein Ende ist eine Lebensleine | mitigated | ESC-1 `socket_af_vsock` in `daemon/crates/ipc/tests/terminal_stream.rs`, Fuzz-Target `control_stream_reader` mit `daemon/crates/sandbox/tests/regressions/`, `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-443) |
| K-30 | der Monitor der virtuellen Maschine der Kernel-Stufe (QEMU `microvm`) | b | hoch | nicht zugelassen bis HUM-449 (`SANDBOX_048`); kein Netzgerät, keine Konsole, kein Monitor, keine Anzeige; volles `-sandbox`; in einem bubblewrap-Gefängnis ohne Fähigkeiten nur mit `/dev/kvm` und der Kernel-Datei; in einer gehärteten vorübergehenden Unit des Nutzers mit `PrivateNetwork`; der Host prüft nach dem Start seine Deskriptoren und seine eigene Startzeile (`SANDBOX_047`) | open | `daemon/crates/sandbox/src/qemu/host_check.rs`, `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-444); Escape-Suite mit Root im Gast (HUM-445) |
| K-31 | die vsock-Tür der Kernel-Stufe | b | hoch | ein vsock-Geräteprozess je Sitzung mit einem Gerät und eigener Gruppe, nie `--forward-cid`; auf dem Host genau ein Lauscher `<uds>_5000`, weitergereicht an den Proxy-Socket der Sitzung; jeder andere Port findet keinen Lauscher; die Host-Prüfung lehnt jeden weiteren Socket im Laufzeitverzeichnis und ein zweites Gerät ab | open | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (`an_extra_listener_beside_the_door_turns_the_host_check_red`, HUM-444) |
| K-32 | der Gastkernel (Root im Gast) | b | hoch | die Garantien werden auf dem Host getragen und gelten deshalb mit Root im Gast; das automatische Laden von Modulen aus, dann die vsock-Module des Plans geladen und das Laden gesperrt, bevor der Agent startet; die Wurzel des Gasts eine nur lesbare Freigabe ohne Laufzeitverzeichnis und ohne das `/proc` des Gefängnisses; beschreibbar nur `/work`, die tmpfs des Profils und `/run`, die der Shim im Gast als tmpfs einhängt, `/run/humanitl` nur lesbar darüber (HUM-466), `mounts.extra_rw` abgelehnt (`SANDBOX_051`) | open | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (HUM-465, HUM-444); Escape-Suite mit Root im Gast (HUM-445) |
| K-33 | die vier Hilfsprozesse einer Sitzung (zwei Dateisystemdienste, das vsock-Gerät, der Emulator) und der erste Prozess der Unit | b, d | hoch | jeder in einem eigenen bubblewrap-Gefängnis mit allen Namensräumen getrennt und ohne Fähigkeit, nur `/usr` des Hosts, was er ausliefert, und sein eigenes Socket-Verzeichnis; der Wurzeldienst nur lesbar mit dem Socket als Deskriptor, der Dienst für `/work` auf dem geprüften Deskriptor mit den Masken des Profils; vom Daemon nichts geerbt außer drei Deskriptoren über eine Übergabe mit einer Verbindung; die Unit endet mit dem Daemon über die Lebensleine, auch bei `SIGKILL`, und mit der Unit des Daemons über `BindsTo` | open | `daemon/crates/sandbox/src/qemu/kvm_tests.rs` (`a_killed_daemon_ends_the_unit_within_five_seconds`, HUM-444) |

K-13 ist der offensichtliche Kanal und der einzige, der rundweg als geschlossen gilt. K-14 steht
hier, damit Angreifer (d) in einer Zeile vorkommt. K-15 kam mit HUM-041: Die Prüfung, die die
Garantien belegt, läuft, wenn der Agent schon gestartet ist. K-16 kam mit HUM-250: Ein Angreifer,
der die Platte füllen kann, kann freigegebenen Verkehr ohne Aufzeichnung hinausgehen lassen.

### K-01 Das Projektverzeichnis `/work`

Der Agent braucht Schreibzugriff auf das Projekt. Was er dort hinterlässt, liegt danach auf der
Platte des Nutzers: eine Datei mit Zugangsdaten im Klartext oder ein Commit, der beim nächsten Push
des Menschen Daten in ein fremdes Repository schiebt. Eingebunden wird nur ein Unterpfad, nie
`$HOME`; `work_mode = "ro"` ist der Vorschlag für Analyse-Sessions. Verzeichnisse wie
`.git/hooks`, `.vscode`, `.idea` und `.github/workflows` werden mit einem leeren `tmpfs` verdeckt,
Dateien wie `.envrc`, `.env`, `.git/config`, `.npmrc` und `Jenkinsfile` mit einer leeren, nur
lesbaren Datei aus einem versiegelten memfd (vollständige Liste in
`profiles/sandbox/default.toml`), damit ein geschriebener Hook beim nächsten Commit nicht auf dem
Rechner läuft. Das Projektverzeichnis und `mounts.extra_ro`/`mounts.extra_rw` werden einmal als
Deskriptoren geöffnet, an dem Ort geprüft, den `/proc/self/fd/N` nennt, und über den Deskriptor
eingebunden (HUM-263); ein Verzeichnis zwischen Prüfung und Mount gegen einen Symlink zu
tauschen, funktioniert also nicht mehr. Der Deskriptor erreicht den Agenten nie. Unter Podman, das
keinen Deskriptor als Quelle eines Einhängens nimmt, erreicht ein solcher Tausch den Container:
Bevor es den Agenten gibt, vergleicht der Shim unter jedem Backend Gerät und Inode von `/work`,
jedem Eintrag aus `mounts.ro`, `mounts.extra_ro` und `mounts.extra_rw` und dem Agentenprogramm mit
dem Paar, das der Starter beim Öffnen der Quelle sah, und lehnt den Start mit `SANDBOX_042` ab
(HUM-437). Unter Podman wird der Tausch damit erkannt und abgelehnt, unter bubblewrap verhindert.
*Restrisiko:* Der
Agent kann im Projekt Daten umordnen, tarnen oder unterbringen, die ein Mensch später hinausträgt;
Humanitl sieht das nur, wenn der Mensch den Diff liest.

### K-02 Die LLM-Durchleitung

Der Agent muss Code an das Modell schicken; diese Anfragen werden nicht angehalten. Der
Modell-Host ist deshalb der zweite offene Kanal und, anders als `/work`, einer über das Netzwerk.
Die Regel ist so eng wie möglich: ein Host, ein Port, ein Schema, `POST` und `GET` und dreizehn
genaue Inferenz-Endpunkte statt der Flächen `/v1/` und `/api/` (HUM-261); ein Pfad mit
`..`-Segmenten wird angehalten (HUM-204). Der Verkehr wird aufgezeichnet und gescannt (`LLM_005`;
jeder Fund bleibt an seiner Anfrage, die Warnung wiederholt sich nur für Funde, die im Lauf neu
sind oder vor 15 Minuten oder mehr zuletzt gewarnt wurden; ein abgelaufener oder verdrängter
Fingerabdruck warnt erneut, HUM-354), und mit `hold.hard_block_checksum_secrets` wird ein durch
Prüfsumme bestätigtes Secret blockiert (`HOLD_004`). Der Zugangsschlüssel eines Anbieters
(HUM-140) liegt im Schlüsselbund des Systems (unter macOS im Anmelde-Schlüsselbund, HUM-349; das
Seatbelt-Profil erreicht kein `securityd` und kein `~/Library/Keychains`, der Agent kann also nicht
danach fragen) und erreicht den Agenten nur als Datei in der
Sandbox; in der Aufzeichnung und in einem Fund ist er überall verdeckt, außer in `Authorization`
gegenüber `llm.endpoint`. Bei mehreren gespeicherten Modellservern (HUM-289) hat jeder einen
eigenen Eintrag im Schlüsselbund, und nur der Ursprung des aktiven Servers bekommt einen Schlüssel
und ist die Durchleitung der nächsten Session; eine Probe schickt den Schlüssel eines
gespeicherten Servers nur an dessen eigenen Ursprung, nie an den eines anderen; ein Endpunkt aus
der Umgebung oder der Kommandozeile über dem aktiven Server, ein geteilter Verweis oder ein
fehlender aktiver Server lassen den Schlüssel ungesendet. Seit HUM-400 ist die Grenze der
gespeicherte Server, den die Session aufgelöst hat: `llm.server` aus dem Lauf, dem Projekt-Profil
oder einem Profil der Session geht `llm.active` vor, nur dieser Server ist die Durchleitung und
bekommt seinen Schlüssel, und ein Name ohne gespeicherten Eintrag lehnt den Start ab
(`CONFIG_021`), statt zurückzufallen. *Restrisiko:* vollständig.
Wer den Agenten über Prompt-Injection lenkt, kann Projektinhalte in einen Prompt legen. Ist
`llm.endpoint` ein Name, kann, wer diesen Namen kontrolliert, die Durchleitung zwischen zwei
Anfragen umlenken, und weil die Regel `allow_private` trägt, sind der Router oder
`169.254.169.254` gültige Ziele; nimm eine Adresse oder `resolver.overrides`. Der Agent kann den
Schlüssel lesen und anderswohin schicken, wo er ein Fund ist, aber nur bei einem Host ohne
erlaubende Regel angehalten wird. Ein geklontes Repository kann mit `llm.server` in seinem
Projekt-Profil einen anderen der gespeicherten Server des Nutzers wählen und so den Modellverkehr
seiner eigenen Session dorthin lenken, etwa an einen Firmenserver statt an einen lokalen; es kann
keinen Server, keinen Endpunkt und keinen Schlüssel festlegen, und die Wahl steht mit ihrer
Herkunft im Kopf der Sandbox, in `humanitl sandbox plan` und beim Start von `humanitl run`. Eine
Rückfrage gibt es nicht; speichere keinen Server, den kein Repository erreichen darf.

### K-03 Das Socket-Verzeichnis des Proxys

Bände Humanitl das Verzeichnis ein, das den Socket enthält, wäre jede Datei, die dort später
entsteht, in der Sandbox sichtbar, und der Agent könnte dort Sockets anlegen, über die zwei
Prozesse der Sandbox an der Aufzeichnung vorbei miteinander sprechen. Eingebunden wird die
Socket-**Datei**, das Verzeichnis auf dem Rechner hat den Modus `0700` und ist vom gRPC-Socket des
Daemons getrennt, der in der Sandbox nicht existiert. Die Isolationsprüfung zählt die Sockets in
der Sandbox. *Restrisiko:* ein Fehler im Proxy selbst; wer den Proxy-Prozess übernimmt, hat das
Netzwerk des Rechners, weshalb der Daemon in Rust geschrieben und gehärtet ist.

### K-04 Von seccomp erlaubte TCP-Sockets

Der Filter muss `socket(AF_INET|AF_INET6, SOCK_STREAM)` erlauben, weil der Agent so den Proxy
erreicht. Garantie 3 heißt deshalb nicht „keine Sockets", sondern „keine anderen". Die Sicherheit
dieses Sockets trägt der Namespace: nur `lo`, eine leere Routing-Tabelle, keine Capabilities, also
endet `connect()` zu einer LAN-Adresse in `ENETUNREACH`. Der Elternprozess des Shims, der die Bridge
hält, ist der einzige Prozess, dem `AF_UNIX` erlaubt ist; er trägt einen Filter mit derselben
Sperrliste plus dieser einen Familie und ist seit HUM-138 nicht dumpbar, sodass `ptrace`,
`/proc/<pid>/mem`, `/proc/<pid>/fd` und `pidfd_getfd(2)` dem Agenten auch bei gleicher UID
verweigert werden. Beide Filter verweigern `seccomp()` mit `SECCOMP_FILTER_FLAG_NEW_LISTENER`.
Behoben mit HUM-203 (Fund M1 vom 2026-09-23): PID 1 war früher das ungefilterte, dumpbare Init von
bwrap, beschreibbar über `/proc/1/mem` bei `ptrace_scope = 0`; jetzt ist der Shim PID 1, gefiltert
und nicht dumpbar, und er weigert sich, den Agenten zu starten, wenn sich das Flag nicht setzen
lässt. *Restrisiko:* Ein Loopback-Dienst, den der Agent selbst startet, ist für ihn erreichbar; das
ist Kommunikation zwischen zwei Prozessen, die beide dem Angreifer gehören, kein Egress.

### K-05 Unix-Sockets des Rechners

Ein falscher Mount macht alles zunichte: `/tmp/.X11-unix` gibt Tastatur und Bildschirm, der
Wayland-Socket dasselbe, D-Bus startet Prozesse auf dem Rechner, `/var/run/docker.sock` ist Root
auf dem Rechner. Das Profil ist eine Erlaubnisliste; die vollständige bwrap-Zeile ist in
`humanitl sandbox argv` sichtbar; der Argument-Builder verweigert `$XDG_RUNTIME_DIR`, `/tmp`,
`/run`, `/home`, `~/.ssh`, `~/.gitconfig`, `~/.netrc` und diese Sockets (`SANDBOX_006`); die
Isolationsprüfung sucht in der laufenden Sandbox nach unerwarteten Sockets; bevor der Agent
startet, vergleicht der Shim die Mounttabelle in der Sandbox Eintrag für Eintrag mit der, die der
Plan erwartet, und eine Einhängung, die nicht im Plan steht (mit oder ohne Socket darin), lehnt den
Start mit `SANDBOX_049` ab (HUM-437); und nach dem Filter wird `AF_UNIX` ohnehin verweigert.
*Restrisiko:* ein Nutzer, der ein eigenes Profil schreibt und einen Datei-Mount (keinen Socket) wie
`~/.ssh` nur lesbar hinzufügt; dieser Mount steht im Plan, und keine der beiden Prüfungen lehnt ihn
ab.

### K-06 PID- und IPC-Namespace

Ohne eigenen PID-Namespace sähe der Agent jeden Prozess des Nutzers und läse Tokens aus
`/proc/<pid>/environ`; ohne eigenen IPC-Namespace erreichte er gemeinsamen Speicher und
Nachrichtenwarteschlangen. Die Sandbox hat eigene PID-, IPC- und UTS-Namespaces, ein frisches
`/proc`, `--new-session` (kein gemeinsames steuerndes Terminal, also keine Injektion über
`TIOCSTI`) und `--die-with-parent`. `--new-session` bleibt, obwohl eine Session am
Pseudo-Terminal des Daemons jetzt ein steuerndes Terminal hat (HUM-280): Es löst die Sandbox von
dem Terminal, das bwrap gestartet hat, und erst danach übernimmt das Kind des Shims das eigene
Pseudo-Terminal des Daemons mit `setsid` und `TIOCSCTTY` (Argument 0, nie stehlend). Das Terminal
des Nutzers wird einer solchen Session nie übergeben, sodass `TIOCSTI` nur in die Eingabe des
eigenen Pseudo-Terminals der Sandbox schreiben kann, dessen Master der Daemon hält; eine Sandbox
mit geerbten Deskriptoren fragt nie nach einem steuernden Terminal, und ein Terminal, das schon
das steuernde Terminal einer Session ist, lässt sich ohne `CAP_SYS_ADMIN` im ursprünglichen
User-Namespace nicht übernehmen. PID 1 ist der Shim
(HUM-203), der verwaiste Prozesse einsammelt und `SIGTERM` und `SIGHUP` weiterreicht; er ist nicht
dumpbar, also sind `/proc/1/environ` und `/proc/1/mem` für den Agenten nicht lesbar. Wo ein
Backend die Sandbox vom Daemon abgekoppelt startet und `--die-with-parent` kein Gegenstück hat
(Podman, dessen Container unter `conmon` weiterläuft, wenn der Client getötet wird), tritt die
Lebensleine des Shims an seine Stelle (HUM-430): das Leseende einer Pipe, deren einziges
Schreibende der Daemon mit `CLOEXEC` hält; endet der Daemon, auch durch `SIGKILL`, sieht PID 1
`POLLHUP` und beendet die Sandbox wie bei `SIGTERM`, nach `KILL_GRACE` mit `SIGKILL` an jeden
Prozess des Namespace. Der Agent hält die Lebensleine nie.
*Restrisiko:* keines bekannt, solange bwrap die Optionen wie dokumentiert umsetzt.

### K-07 `/proc` und `/sys`

Selbst ein eigenes `procfs` verrät Kernel-Version, Module und CPU-Details; `/sys` würde Hardware
verraten und, falls eingebunden, Schreibpfade in den Kernel. `/proc` gehört dem neuen Namespace,
`/sys` ist nicht eingebunden. *Restrisiko:* Kernel-Informationen bleiben lesbar; für einen
Angreifer, der schon Code ausführt, ist das kein Gewinn.

### K-08 Symlinks aus `/work` hinaus

Der Agent legt `/work/x -> /home/user/.ssh` an. Innerhalb der Sandbox führt der Link ins Leere,
weil es `/home` dort nicht gibt. Gefährlich wird er **nach** der Session, auf dem Rechner, wenn ein
Werkzeug des Nutzers ihm folgt. Die Zusammenfassung der Session markiert jeden neuen Symlink,
dessen Ziel außerhalb von `/work` liegt (`SANDBOX_022`), und der Daemon liest den Baum mit
`openat2` und `RESOLVE_BENEATH |
RESOLVE_NO_SYMLINKS`, ohne je einem Link zu folgen. *Restrisiko:* Der Nutzer übersieht die
Markierung.

### K-09 Terminal-Escapes

Die Ausgabe des Agenten ist vom Angreifer beeinflusst. OSC 52 schreibt in die Zwischenablage des
Rechners, OSC 8 versteckt eine fremde URL hinter harmlosem Text, Titel-Escapes verändern das
Fenster. Der Daemon filtert, bevor irgendein Byte einen Client erreicht
(`humanitl_core::TerminalFilter`, Richtlinien `ColourOnly` und `FullScreen`), in drei
Schreibweisen jedes Einleiters und nur als wohlgeformtes UTF-8; der Ringpuffer für spät
hinzukommende Clients hält gefilterte Bytes; das Widget registriert keinen Handler für die
Zwischenablage und verweigert Schreib- und Lesezugriffe über OSC 52 ausdrücklich, als zweite Wand
hinter dem Filter (HUM-313); Farbabfragen, die der Filter durchlässt (OSC 4, 10, 11,
`CSI ? 996 n`), beantwortet der Emulator mit den Farben des Themes. *Restrisiko:* ein Fehler in
Filter oder Widget, und Text, der nur wie eine Meldung von Humanitl aussieht; deshalb steht die
Aussage „eine Anfrage wartet" in einem Streifen über dem Terminal, gezeichnet aus dem
Ereignisstrom.

### K-10 DNS und der Zeitpunkt der Auflösung

Ein Hostname trägt bis zu 63 Bytes pro Label. Wer nach `<encoded data>.attacker.example` fragt, hat
ausgeleitet, sobald irgendjemand den Namen auflöst. Humanitl löst erst **nach** der Freigabe auf,
einmal, über den Port `Resolver`, und hält die Adresse fest; private, Loopback-, Link-Local- und
CGNAT-Adressen werden verweigert, es sei denn, die Regel trägt `allow_private`. *Restrisiko:* Nach
der Freigabe geht der Name naturgemäß hinaus, und der *Zeitpunkt* einer Freigabe bleibt
beobachtbar, ein langsames Ein-Bit-Signal, wenn der Mensch bereitwillig freigibt.

### K-11 Metadaten von Warteschlange und Regeln

Der Agent lernt aus dem Verhalten, was erlaubt ist: Eine sofortige Antwort bedeutet eine Regel,
eine `403` mit `reason: rule` eine ausdrückliche Blockade, eine lange Verzögerung einen Menschen,
der hinschaut. Der Proxy beantwortet außerdem den virtuellen Host `humanitl.internal` mit Status und
Regeln. Dieser Host ist nur lesbar und wird ohne Upstream beantwortet; `/ask` legt eine Karte im
Fenster an, nie eine Regel. *Restrisiko:* Das Orakel bleibt, absichtlich: Ein Agent, der weiß, dass
PyPI erlaubt und GitHub blockiert ist, stört den Menschen seltener.

### K-12 Paket-Caches

Ein über Projekte hinweg geteilter Cache (`~/.npm`, `~/.cache/pip`, `~/.cargo`) trägt Daten von
einem Projekt in ein anderes. Caches gibt es je Projekt, und wer keinen konfiguriert, bekommt
keinen. *Restrisiko:* Wer einen geteilten Cache hinzufügt, um Zeit zu sparen, baut den Kanal
wieder auf.

### K-13 Direkter Egress ins Netzwerk

Eine TCP-Verbindung ins Internet, ein UDP-Paket, ein QUIC-Handshake, ein ICMP-Tunnel, ein
Raw-Socket: nichts davon existiert. Es gibt kein Interface außer `lo`, keine Route, keine
Capability, eine anzulegen, und der Filter verweigert `SOCK_DGRAM`, `SOCK_RAW` und jede Familie
außer `AF_INET`/`AF_INET6`. Verweigerte `socket()`-Aufrufe werden seit HUM-138 gezählt und
angezeigt; ein `connect()`, das an der leeren Routing-Tabelle scheitert, nicht.
*Restrisiko:* ein Fehler im Kernel.

### K-14 Die Audit-Datei auf dem Rechner

Wer die JSONL-Datei ändert, ändert die Antwort auf „was ging damals hinaus". Jeder Eintrag trägt
`seq`, `ts`, den Hash des Vorgängers und seinen eigenen über kanonisches JSON, versiegelt mit HMAC;
alle `audit.anchor_every` Einträge und beim Herunterfahren steht ein Anker in der Datei und in
`audit_anchors`. *Restrisiko:* Ein Angreifer, der als derselbe Nutzer läuft, hat Schlüssel, Datei
und Datenbank und kann alles neu bauen; ein Abschneiden hinter dem letzten Anker wird nicht
erkannt; die Aufbewahrung löscht den Anfang der Kette, dokumentiert durch ein versiegeltes
`audit.pruned` (Einzelheiten in
[`SECURITY.md`](SECURITY.md#8-the-audit-log)).

### K-15 Das Zeitfenster zwischen `exec` und `SIGKILL`

Der Shim meldet die drei Garantien und startet den Agenten gleich danach; er erzwingt nichts
(`CONVENTIONS.md` 4.12). Zwischen dem `exec` des Agenten und dem `SIGKILL` des Daemons läuft der
Agent in einer Sandbox, deren Isolation noch nicht belegt ist: Die Bridge steht, der Proxy nimmt an,
eine `allow`-Regel leitet ohne Menschen weiter, die Durchleitung reicht durch. Im Fall von
`SANDBOX_015` ist der zweite Socket, den die Prüfung bemängelt, in diesem Zeitfenster benutzbar. Der
Daemon liest den Bericht sofort und beendet ohne Schonfrist; die Obergrenze liegt bei 10 s im
Daemon und 15 s für `humanitl sandbox run` und `escape-launch`. Jede Anfrage, über die keine Regel
entscheidet, wartet weiterhin auf einen Menschen. Das Fenster zu schließen hieße, dass der Proxy
Verbindungen verweigert, bis die Isolation belegt ist; das ist ein eigenes Issue.
`a_second_socket_in_the_project_stops_the_start`
in `daemon/crates/ipc/tests/sandbox_start.rs` belegt, dass die Session gestoppt und nie als laufend
gemeldet wird; dass im Zeitfenster nichts hinausgeht, ist **nicht** belegt.

### K-16 Ausfall der Aufzeichnung

Kann der Recorder seine Datenbank nicht öffnen oder nicht schreiben (Platte voll, Datenbank
gesperrt, Datei nur lesbar), gehen freigegebene Anfragen und die Durchleitung trotzdem hinaus,
werden aber nicht aufgezeichnet. Die Angreifer (a) bis (c) können das herbeiführen, indem sie von
`/work` aus die Platte füllen. Der Daemon schickt `RECORDER_001` oder `RECORDER_003` an jeden
Client einmal je Ursache, und zuerst an einen Client, der sich während des Ausfalls verbindet; die
Statusleiste wird rot, jede neue Karte sagt „Wird nicht aufgezeichnet", und die erste Freigabe der
Session wartet auf eine Bestätigung. `RECORDER_005` beendet den Ausfall. Verlorene Flows werden
nicht nachträglich geschrieben. *Restrisiko:* Verkehr während des Ausfalls, vor allem über
`allow`-Regeln und die Durchleitung, den kein Mensch sieht, hat keine Aufzeichnung; die Aussage
„beides wird aufgezeichnet" ist für diese Zeit sichtbar falsch.

### K-17 Das Agentenprogramm aus einem Verzeichnis des Nutzers

Liegt der Agent in `~/.opencode/bin`, `~/.local/bin` oder `~/bin` (oder zeigt ein absolutes
`agent.command` außerhalb der Einbindungen), bindet der Daemon genau diese eine Datei nur lesbar
unter `/run/humanitl/agent/<name>` ein (HUM-278). Von innen können die Angreifer (a) bis (c) die
Datei lesen und ausführen, die sie ohnehin schon als der Agent ausführen; schreiben können sie sie
nicht (nur lesbar eingebunden), und sie sehen keine andere Datei des Verzeichnisses. Der Daemon
bindet nur ein natives ELF-Programm ein, das dem Nutzer oder Root gehört und dessen Datei und
Verzeichnis niemand sonst beschreiben kann (Gruppenschreibrecht nur für eine Gruppe, die
nachweislich die private Gruppe des Nutzers ist). Der Launcher öffnet die Datei einmal mit
`O_PATH | O_NOFOLLOW`, prüft Eigentümer, Modus und ELF-Header über diesen Deskriptor und bindet den
Deskriptor ein, sodass ein Tausch des Pfads oder eines übergeordneten Verzeichnisses nach der
Prüfung nicht ändert, was die Sandbox bekommt. Ein Befehl, der nach `--` angegeben wird, wird nie
auf diese Weise eingebunden. Seit HUM-280 bekommt eine Session mit einem solchen Befehl
(`humanitl run -- bash`) trotzdem die Einbindung des konfigurierten Agenten daneben, nach denselben
Regeln, und `/run/humanitl/agent` an erster Stelle im `PATH`; das ist dieselbe Datei, die ein vom
Daemon gestarteter Agent bekäme, und der eigene Befehl des Aufrufers bleibt uneingebunden.
*Restrisiko:* Wer als der Nutzer schreiben kann oder vor der Prüfung Schreibzugriff auf ein
übergeordnetes Verzeichnis hat, kann eine andere Datei unterschieben, die dem Nutzer oder Root
gehört; das liegt außerhalb des Bedrohungsmodells (siehe [Außerhalb des Umfangs](#out-of-scope)).

### K-18 Die Tür für weitere Shells

`humanitl sandbox exec` (RPC `Exec`, HUM-299) öffnet eine Shell neben dem Agenten. Der Daemon
betritt die Namespaces der Sandbox nicht; er fragt PID 1 über ein `SOCK_SEQPACKET`-Paar, das der
Launcher nur für eine Session am Pseudo-Terminal des Daemons anlegt, und der Shim forkt die Shell
von innen. Dieses Paar ist neben dem Proxy-Socket ein zweiter Kanal zwischen der Sandbox und dem
Rechner, also würde Angreifer (b) drei Dinge versuchen. (1) *Von innen eine Shell anfordern oder
dem Daemon etwas über das Paar schicken.* Das Paar hat in keinem Dateisystem einen Namen; innen
hält nur PID 1 sein Ende (das Kind des Agenten schließt es, es trägt `CLOEXEC`, jede weitere Shell
schließt alle Deskriptoren über 2), und PID 1 ist nicht dumpbar, also erreichen `/proc/1/fd` und
`pidfd_getfd` es nicht. Selbst eine Anforderung von innen würde nichts starten, was der Agent nicht
schon selbst ausführen kann. Der Daemon liest nur Antworten von neun Bytes für IDs, nach denen er
gefragt hat, und nimmt nie einen Deskriptor aus dem Paar an. (2) *Eine Shell mit weniger
Beschränkungen als der Agent bekommen.* Die Shell erbt den Filter von PID 1, der `AF_UNIX` erlaubt,
und installiert vor dem `exec` den Filter des Agenten obendrauf; der Shim prüft, dass
`socket(AF_UNIX)` verweigert wird, und startet die Shell andernfalls nicht. Mounts, Namespaces,
Umgebung und Proxy sind die des Agenten, weil die Shell innerhalb derselben Sandbox geforkt wird.
(3) *Die Session überleben oder den Rechner erschöpfen.* Höchstens vier Shells leben gleichzeitig
(Shim und Daemon zählen beide); PID 1 beendet sie, wenn der Agent endet, sie sterben mit dem
forkenden Thread und dem PID-Namespace, und der Daemon beendet ihre Terminals, wenn sein Ende des
Paars schließt. Der einzige Aufrufer auf der Seite des Rechners ist der RPC `Exec`, hinter dem
Token des Daemons auf dem Socket des Daemons, der nie in die Sandbox eingebunden wird; Shell-IDs
tragen einen Teil der Sandbox-ID, also erreicht eine ID aus einer früheren Session nichts.
*Restrisiko:* Verweigerte `socket()`-Aufrufe einer weiteren Shell werden verweigert, aber nicht
gezählt (K-13 bleibt nur für den Agenten sichtbar), weil die Filter einen zweiten Listener
verweigern. Die Shell ist gegenüber dem Agenten nicht vertraulich: Sie läuft mit der UID des
Agenten im selben PID-Namespace ohne `hidepid`, also kann der Agent lesen, was darin getippt wird,
und in ihr Terminal schreiben. Tippe dort nie Secrets ein. Die Standard-Shell führt keine
Startdateien aus (`bash --norc --noprofile`, `sh` ohne `ENV`), sodass eine rc-Datei, die der Agent
untergebracht hat, beim Öffnen nicht läuft. Weitere Shells vom Agenten zu isolieren (ein
untergeordneter User-Namespace mit einer anderen UID oder `hidepid`) ist ein Kandidat für ein
späteres Issue.

*Im Strom-Modus (HUM-443, K-29)* überquert kein Deskriptor die Grenze: Der Daemon schickt einen
Rahmen zum Öffnen mit Befehl und Größe über den Steuerkanal, der Shim öffnet das Pseudo-Terminal
der Shell selbst und forkt die Shell wie oben, und Bytes, Größe, Ende und Auflegen der Shell
reisen als Rahmen ihrer Kennung. Die Grenzen sind dieselben: der Filter des Agenten obendrauf,
belegt durch verweigerte `AF_UNIX`- und `AF_VSOCK`-Sockets, höchstens vier, beendet, wenn der
Strom oder die Sandbox endet. Ein Rahmen für eine Kennung, die der Daemon nicht geöffnet hat,
ändert auf keiner Seite etwas.

Die Kanäle K-19 bis K-23 gibt es nur auf macOS, wo die Sandbox Seatbelt statt bwrap ist
([ADR-0019](adr/0019-seatbelt-on-macos.md), [`SECURITY.md` Abschnitt 12](SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap)).

### K-19 Das gemeinsame Loopback von macOS

Auf macOS gibt es keinen Netzwerk-Namespace, also lauscht die Tür des Shims auf einem
Loopback-Port des Rechners, frei gewählt je Session. Angreifer (b) würde versuchen, von innen
andere Loopback-Dienste des Rechners zu erreichen; das Profil erlaubt ausgehendes Loopback nur zu
den eigenen Ports der Session, und `tests/escape/macos/run.sh` prüft, dass ein anderer Port verweigert
wird. In der anderen Richtung kann sich jeder lokale Prozess jedes Kontos mit der Tür verbinden und
erreicht den Proxy, als wäre er der Agent. *Restrisiko:* Ein solcher Prozess sendet mit den Regeln
der Session: Was eine `allow`-Regel oder die Durchleitung durchlässt, geht ohne Anhalten hinaus, der
Rest landet in der Warteschlange des Besitzers, und der Mensch sieht es als Anfrage des Agenten.
HUM-331 beschränkt den Proxy auf den Shim der Session.

Das Profil erlaubt außerdem einen zweiten, zufälligen Port der Session, auf dem ein Agent seinen
eigenen lokalen Server betreiben darf (HUM-365): OpenCode 2 spricht mit einem, und ein fester Port
ließe die Sandbox an einen Dienst des Rechners. Seatbelt kennt nur `localhost`, das beide Adressen
umfasst, also ist ein Session-Port auf `127.0.0.1` und `[::1]` frei und nie einer der festen
Dienst-Ports von OpenCode. *Restrisiko:* Jeder lokale Prozess erreicht auch diesen Server, darum
schützt ihn der Adapter mit einem Passwort je Session. Es schützt vor anderen lokalen Prozessen,
nicht vor dem Agenten: Der Wert steht in dessen Umgebung und kann in einer Aufzeichnung landen. Ein
Prozess des Rechners, der diesen Port während der Session auf der anderen Adresse bindet, wird von
innen erreichbar; er müsste dafür genau diesen zufälligen Port treffen.

### K-20 Kein PID-Namespace auf macOS

Der Agent läuft in einer eigenen Session (der Shim ruft dafür `setsid` auf). Am Ende des Befehls
und drei Sekunden nach einem Stopp beendet der Shim die Prozesse dieser Session, in Durchgängen,
die sich wiederholen, bis einer niemanden mehr findet, höchstens zehn. Ein Prozess, der selbst
`setsid` aufruft, verlässt diese Session und überlebt die Sandbox, und das könnte auch einer, der
sich schneller forkt, als die Durchgänge laufen. Er behält das Profil, und das Profil erlaubt ihm
den Proxy-Socket des Daemons direkt, nicht nur die Tür des Shims. *Restrisiko:* Ein solcher Prozess
kann weiter Anfragen schicken, solange der Daemon läuft, über launchd und über Anmeldungen
hinweg; `allow`-Regeln und die Durchleitung gelten, alles andere wird nach dem Ende der Session in
der Warteschlange angehalten. Er kann auch weiter das Projektverzeichnis verändern, was die
Zusammenfassung der Session nicht sieht. HUM-331 beschränkt den Proxy-Socket auf den Shim der
laufenden Session; bis dahin beendet ein Neustart des Daemons den Weg nach draußen für einen
solchen Prozess.

### K-21 Lesen per Sperrliste, Pfade statt Deskriptoren auf macOS

Auf Linux sieht die Sandbox nur die eingehängten Bäume. Auf macOS erlaubt das Profil, alles
außerhalb von `/Users`, `/Volumes`, `/private/var/folders`, `/private/tmp` und `/private/var/tmp` zu
lesen, also liest
Angreifer (b) ganz `/etc` (die echten `passwd` und `hosts`), `/Library`, `/Applications`,
`/opt/homebrew` und `/usr/local/etc`. *Restrisiko:* Zugangsdaten in einer systemweiten
Konfigurationsdatei erreichen den Agenten und über die Durchleitung den Modell-Host, ohne
angehalten zu werden. Bewahre Secrets im Home-Verzeichnis auf, das nicht lesbar bleibt.

Das Profil nennt Pfade. Das Projektverzeichnis wird vor dem Start aufgelöst und gegen die
Richtlinie geprüft, aber nicht über einen Deskriptor festgehalten (HUM-263 auf Linux). Seatbelt
vergleicht aufgelöste Pfade, sodass ein eingetauschter symbolischer Link ins Leere führt; nur ein
lokaler Prozess desselben Kontos, außerhalb der Angreifer dieses Modells, könnte zwischen Prüfung
und Start ein anderes echtes Verzeichnis unter demselben Namen eintauschen. Angreifer (b)
kann das von innen nicht: Das Profil lässt den Agenten nur innerhalb des aufgelösten Projekts,
seines Session-Home und tmp schreiben.

### K-22 Seatbelt ist veraltet

Apple kennzeichnet `sandbox-exec` als veraltet und dokumentiert die Profilsprache nur an
Beispielen;
derselbe Kernel-Mechanismus trägt die Sandboxen von Chrome, Codex und Claude Code, und die
Escape-Suite lief auf macOS 15 (CI) und macOS 27 (Entwicklung) durch. Ein künftiges macOS
könnte sein Verhalten ohne Ankündigung ändern. Die Escape-Suite für macOS läuft bei jedem Commit im
CI-Job `macos` und bei jedem Release gegen die mitgelieferten Programme, auf der macOS-Version der
`macos-15`-Runner von GitHub. Eine Änderung in dieser Version zeigt sich vor einem Release als roter
Lauf. Auf jeder anderen Version misst der Shim die Garantien bei jedem Start, und eine rote Zeile
hält die Session an (K-15). *Restrisiko:* Eine Regression, die die Proben des Shims nicht abdecken,
erreicht Nutzer einer ungetesteten Version unbemerkt.

### K-23 Lokale Seitenkanäle auf dem Rechner unter macOS

Das Profil erlaubt POSIX-Semaphoren und gemeinsamen Speicher sowie die Mach-Dienste für
Nutzerdaten, Benachrichtigungen, das Systemprotokoll und Einstellungen, weil gewöhnliche Werkzeuge
sie brauchen. Jeder davon ist ein Kanal zu Prozessen auf demselben Rechner, nicht ins Netzwerk.
Angreifer (b) könnte darüber Daten an einen kooperierenden Prozess auf dem Rechner übergeben;
dieser Prozess bräuchte dann seinen eigenen Weg nach draußen, den die Sandbox ihm nicht gibt.
*Restrisiko:* Einträge im Systemprotokoll sind für die anderen Prozesse des Nutzers lesbar.

### K-24 Die Wegwerf-Sandbox

Auf macOS (Seatbelt) gibt es kein Overlay; die Schicht ist dort ein APFS-Klon des Projekts
(HUM-374, Einzelheiten und Restrisiken im englischen Original). Der Weg zurück ist derselbe Code.

Mit `sandbox.mode = "disposable"` (HUM-350) ist der Projektordner die nur lesbare untere Schicht
eines Overlays unter `/work`, und die Änderungen des Agenten gehen in eine obere Schicht, die der
Daemon unter `$XDG_RUNTIME_DIR/humanitl/disposable/<id>/` angelegt hat (Modus `0700`, tmpfs). Nach
dem Lauf sieht ein Mensch den Diff durch und übernimmt alles, einen Teil oder nichts davon. Die
obere Schicht wird von Angreifer (b) geschrieben und vom Daemon gelesen; sie zu übernehmen ist ein
Schreibpfad von der Sandbox zurück auf den Rechner, also versucht (b) vier Dinge. (1) *Während des
Laufs ins Projekt oder auf den Rechner schreiben.* Das Projekt ist die untere Schicht und wird nie
beschrieben; `mounts.extra_rw` wird in diesem Modus nur lesbar eingehängt; der Pfad der oberen
Schicht ist in der Sandbox nicht sichtbar (`/run` kommt nie vom Rechner, und bubblewrap öffnet die
Overlay-Verzeichnisse über Deskriptoren). (2) *Weiterschreiben, während der Daemon liest.* Der
Daemon liest und übernimmt erst, nachdem bubblewrap eingesammelt und der erste Prozess der Sandbox
verschwunden ist; mit PID 1 beendet der Kernel den ganzen PID-Namespace, und der Mount-Namespace
des Overlays endet mit seinem letzten Prozess. (3) *Den Speicher des Rechners füllen.* Das tmpfs
des Laufzeitverzeichnisses ist die harte Grenze; der Daemon misst das Wachstum dieses Dateisystems
alle 250 ms und stoppt die Sandbox oberhalb von 2 GiB oder 200.000 Einträgen (oder der Hälfte
dessen, was frei war), `SANDBOX_032`. Zwischen zwei Messungen kann der Agent über die Grenze hinaus
bis zur harten Grenze schreiben. (4) *Die Übernahme woanders hin oder etwas anderes schreiben
lassen.* Jeder Zugriff auf die Schicht und auf das Projekt geht über
`openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)`; einem Symlink, den der Agent in der Schicht
untergebracht hat, wird nie gefolgt, ein Symlink, dessen Ziel das Projekt verlässt, wird nie
geschrieben, und ein Symlink, der inzwischen im Projekt aufgetaucht ist, stoppt das Schreiben
(`ELOOP`). Das Ziel eines Symlinks muss ein schlichter relativer Pfad sein (`..` nur am Anfang,
nicht mehr als die eigene Tiefe des Links), sodass eine Kette wie `sub/d -> ..` und
`e -> sub/d/..` nicht über das Projekt hinausführen kann. Dateien werden mit
`O_CREAT | O_EXCL | O_NOFOLLOW` unter einem eigenen Namen angelegt und mit `renameat` an ihren
Platz verschoben, was einen Symlink am Ziel ersetzt, statt ihm zu folgen. Geräte, FIFOs und
Sockets werden nie geschrieben, setuid-, setgid- und Sticky-Bits fallen weg, Hardlinks werden zu
eigenen Dateien, Whiteouts und undurchsichtige Verzeichnisse löschen nur innerhalb des Projekts,
und Einträge mit Overlay-Attributen, die ein unprivilegiertes Overlay nie schreibt (`redirect`,
`metacopy`), werden verweigert. Der Projektordner wird einmal
vor dem Start geöffnet; der Ausgangsstand wird über diesen Deskriptor erfasst, bevor die Sandbox
startet, der Launcher hängt nur einen Ordner mit demselben Inode ein, und Diff und Übernahme öffnen
den Pfad nie erneut. Eine Datei, die der Mensch seit dem Start geändert hat, ist ein Konflikt und
wird nur geschrieben, wenn er für genau diese Änderung bestätigt wurde (`overwrite_ids`, oder
`--overwrite` für alle ausgewählten, HUM-351), und das Ziel wird unmittelbar vor jedem Schreiben noch einmal verglichen
(ein verbleibendes Zeitfenster von wenigen Systemaufrufen, keine Sperre auf dem Ordner); jede
ausgewählte Änderung wird
vor dem ersten Schreiben geprüft, und eine fehlschlagende Prüfung schreibt nichts. Pfade und Diffs
sind Daten des Agenten und werden bereinigt angezeigt. *Restrisiko:* Was der Mensch übernimmt, ist
der Inhalt des Agenten, wie bei einer direkten Session; sieh den Diff durch, besonders Dateien,
die der Rechner von selbst ausführt (Hooks, `Makefile`, Skripte in `package.json`, die der Fund
`SANDBOX_026` der Zusammenfassung bisher nur für direkte Sessions nennt). Anfragen, die der Agent
über den Proxy geschickt hat, sind draußen, was auch immer mit der Schicht geschieht. Die
Konfliktprüfung vergleicht Typ, Größe, `mtime` und Modus, nicht den Inhalt: Eine Bearbeitung durch
den Menschen, die alle vier beibehält, fällt nicht auf. Ein Daemon, der endet, verwirft eine
ausstehende Schicht; ihre Änderungen gehen dann verloren, und die Aufzeichnung sagt das.

*Podman (HUM-438).* Das Overlay-Volume der Engine mit eigener oberer Schicht ist eine zweite Form
der oberen Schicht, gemessen und nicht verwendet. Es nimmt das Projekt als Pfad, den die Engine
später auflöst; ein Deskriptor ist keine Quelle (HUM-292 P13, und P10 mit `/proc/self/fd/N` als
untere Schicht), und im Container lässt sich das Overlay nicht über Gerät und Inode mit dem
geprüften Ordner verbinden. Ein Angreifer der Art (b), ein Prozess desselben Nutzers, oder alles andere, was den Ordner
umbenennen kann, könnte ihn
zwischen Prüfung und Einhängen tauschen und einen ungeprüften Ordner zur unteren Schicht machen.
Unter Podman lehnt `disposable` deshalb mit `SANDBOX_030` ab, und `auto` läuft direkt mit
`SANDBOX_033`; ein solcher Lauf hat keine der Schutzmaßnahmen dieses Abschnitts. ESC-5 prüft die
Ablehnung unter Podman.

### K-27 Die Container-Engine

Unter dem Podman-Backend (HUM-433, [SECURITY.md Abschnitt 13](SECURITY.md#13-linux-podman)) ist
die Sandbox ein rootless Container, und die Engine, die ihn baut, gehört zur vertrauenswürdigen
Basis: Podman, `conmon`, das Elternprozess des Containers bleibt, die OCI-Laufzeit `crun`, der
Container-Speicher des Nutzers unter `~/.local/share/containers` und die Konfiguration, die Podman
liest. Ein Agent (Angreifer b), der in einem davon eine Schwäche fände, verließe die Sandbox wie
über eine Schwäche von bubblewrap; ein Prozess desselben Nutzers (Angreifer d) könnte dessen
eigene `containers.conf`, `mounts.conf` oder Hooks ändern und jedem Container ein Einhängen, ein
Gerät, ein Netz oder einen Hook hinzufügen. Die Konfigurationsquellen von Podman sind
`containers.conf` in jeder Ebene (`/usr/share/containers`, `/etc/containers`,
`~/.config/containers`, `CONTAINERS_CONF_OVERRIDE`), `mounts.conf` an denselben drei Orten und über
`--default-mounts-file`, die Verzeichnisse der OCI-Hooks, `storage.conf`, `registries.conf`,
`policy.json` und die entfernten Verbindungen (`CONTAINER_HOST`, `CONTAINER_CONNECTION`). Die
Bindung: Der Daemon startet Podman mit eigener
`containers.conf`, `storage.conf` und `registries.conf` und einem leeren Hook-Verzeichnis, mit
leerer Umgebung außer `HOME`, dem `XDG_RUNTIME_DIR` des Nutzers, `PATH`, dem Sitzungsbus des Nutzers und diesen drei Pfaden; die Zeile trägt
jedes Flag, auf dem die drei Garantien ruhen, ohne Schlüssel, der es ändert; die Umgebung des
Agenten geht nur durch den versiegelten memfd und nie in die gespeicherte Konfiguration des
Containers; die Deskriptoren des Shims (Bericht, Lebensleine, Umgebung, Mounttabelle, Tür zu
weiteren Shells, Terminal) erreichen ihn über einen Socket der Sitzung, der genau eine Verbindung
mit der Kennung des Daemons und einem Einmal-Zeichen annimmt und geschlossen ist, bevor der Agent
startet, sodass der Client der Engine sie nie hält (HUM-434); unter der gehärteten Unit des
Daemons läuft jeder Aufruf von Podman als vorübergehende Unit `humanitl-*` des Dienstverwalters
des Nutzers, und das Beenden einer Sitzung stoppt auch ihre Unit; der Container endet über die
Engine beim Namen und, wenn der Daemon stirbt, über die Lebensleine; was eine Konfigurationsquelle trotzdem hinzufügt, findet der Shim in der
Mounttabelle, bevor der Agent startet, und lehnt es mit `SANDBOX_049` ab, ob es einen Socket trägt
oder nicht (HUM-437). *Restrisiko:* Die gemeinsame Stufe teilt den Kern des Hosts wie bubblewrap;
eingehängt wird über Pfade, ein getauschter Pfad wird von der Prüfung von Gerät und Inode durch den
Shim nach dem Einhängen erkannt und abgelehnt (`SANDBOX_042`, HUM-437), nicht verhindert; die
eigenen Dateien der Engine `/run/.containerenv` und `/etc/hostname` werden über Einhängepunkt,
Typ, Modus und Flags erkannt, nicht über ihre Quelle (die Masken unter `/proc` und die
Gerätedateien sind über Gerät und Inode an `/dev/null` und die Geräte des Rechners gebunden);
Podman liest `/etc/containers/policy.json` und den Speicher, den es mit dem Nutzer teilt. HUM-448 hat die Stufe zugelassen, nachdem der CI-Job unten auf `main` grün war; ein Mensch startet sie nur, wenn er sie wählt, `auto` bleibt bubblewrap, und die Kernel-Stufe lehnt jeder Start bis HUM-449 mit `SANDBOX_048` ab. Geprüft von ESC-1, ESC-2 und ESC-5 im CI-Job
`escape-tests-podman` (HUM-435): dieselben Proben wie unter bubblewrap, dazu ein leeres
`/run/.containerenv`, kein Socket von `conmon` oder der Engine, der Shim als PID 1 mit dem Filter,
und ein Eintrag in `mounts.conf`, ein Volume und ein Hook in der eigenen Engine-Konfiguration des
Runners, die einen gewöhnlichen Container erreichen und die Sandbox nicht.

### K-28 Das Abbild als Wurzel der Sandbox

Unter Podman mit `rootfs = "image"` (HUM-439, [SECURITY.md Abschnitt 13](SECURITY.md#ein-abbild-als-wurzel-hum-439))
ist die Wurzel der Sandbox ein OCI-Abbild, und mit ihm kommen drei Dinge. **Herkunft:** Das Abbild
ist Code, den jemand anderes gebaut hat, wie eine Abhängigkeit (Angreifer b), und es läuft als der
Agent; wer seine Registry oder seinen Namen beherrscht, bestimmt, was ein späteres Ziehen bringt.
**Digest:** Ein Name kann morgen auf ein anderes Abbild zeigen; ein Start, der den Namen nähme,
führte etwas aus, das niemand gewählt hat. **Das Netz beim Ziehen:** Die Engine holt die Schichten
selbst, über das Netz des Daemons, am Proxy, seinen Regeln und seiner Aufzeichnung vorbei. Die
Bindung: Ein Abbild wird nur mit Digest genannt (`name@sha256:...`, sonst `CONFIG_003`); vor jedem
Start fragt der Plan die Engine nach genau diesem Digest und lehnt ein fehlendes Abbild oder einen
anderen Digest unter demselben Namen mit `SANDBOX_043` ab; kein Start zieht (`--pull=never`); nur
`humanitl sandbox image pull` zieht, auf Wunsch eines Menschen, und Kommandozeile und Oberfläche
sagen vorher, dass das Netz dabei direkt genutzt wird; ein Name ohne Registry wird abgelehnt, und
die eigene `registries.conf` des Daemons hat keine Suchliste und keinen Spiegel; Projektordner,
Tür, CA-Zertifikat und Bundle, Shim, die Dateien des Adapters und das Sitzungsverzeichnis kommen
vom Host, nie aus dem Abbild, also steht eine CA, ein Schlüssel oder ein Sitzungsverzeichnis nie in
einem Abbild; die Zeile setzt Nutzer, Einstiegspunkt, `--image-volume=ignore` und
`--no-healthcheck` selbst, und ein Abbild, dessen Umgebung einen Namen mit `HUMANITL_` setzt, der
in der Engine über die Variablen des Shims ginge, wird abgelehnt. Zugangsdaten zur Registry kommen
beim Ziehen nur aus `<config>/humanitl/podman/auth.json`: Der Daemon öffnet sie, ohne einem Link
zu folgen, prüft Besitzer und Rechte an der offenen Datei, lehnt `credHelpers` und `credsStore` ab
und reicht der Engine unter `--authfile` eine private Kopie, die nach dem Ziehen gelöscht wird;
eine Ergänzung zur `registries.conf`, die `credential-helpers` setzt, lehnt das Ziehen ab
(HUM-456). *Restrisiko:* Die Signaturregel
nimmt jedes Abbild an, der Digest ist der einzige Anker; die Prüfung der Ergänzungen läuft, bevor
die Engine sie liest, also könnte ein Prozess desselben Kontos dazwischen eine anlegen, aber ein
solcher Prozess liest Schlüsselbund und Zugangsdateien auch direkt, und die Prüfung schützt vor
einem versehentlich eingerichteten Helfer; die Prüfung des Digests liest die
Metadaten der Engine und rechnet die entpackten Schichten nicht neu nach, also kann ein Prozess
desselben Kontos (Angreifer d), der in den Container-Speicher des Nutzers schreibt, den Inhalt
unter unverändertem Digest ändern; die Annotationen des Abbilds erreichen die Engine; das Ziehen
selbst zeichnet nicht der Proxy auf, nur das Protokoll des Daemons. Geprüft von ESC-2
`ca_is_the_hosts` (die CA in der Sandbox ist die Datei des Daemons, nach Gerät und Inode), das
unter jedem Backend läuft und mit `HUMANITL_ESCAPE_IMAGE` unter einem Abbild, und von den Unit- und
Live-Tests in `daemon/crates/sandbox/src/podman/` (ein anderer Digest unter demselben Namen
abgelehnt, nichts gezogen).

### K-29 Der Steuerkanal der Terminals

In einer micro-VM überquert kein Deskriptor die Grenze: Der Daemon kann dem Agenten kein
Pseudo-Terminal geben, `SCM_RIGHTS` kann die Untertanenseite einer weiteren Shell nicht tragen
(K-18), und kein Signal des Hosts erreicht einen Prozess des Gasts. Im Strom-Modus (HUM-443)
öffnet der Shim jedes Pseudo-Terminal selbst, und Terminal-Bytes, Größen, weitere Shells,
Exit-Codes, Unterbrechungen und Lebenszeichen reisen als Rahmen über einen Strom-Socket
(`daemon/bin/humanitl-shim/src/frame.rs`, die eine Datei, die beide Seiten übersetzen). Die
Kernel-Stufe nutzt ihn immer; unter bwrap kann ein Aufrufer ihn verlangen, damit beide Modi
dieselben Terminal-Tests bestehen. Er ist ein zweiter Kanal zwischen Sandbox und Host neben der
Tür, also würde Angreifer (b) vier Dinge versuchen. (1) *Ihn als Weg nach draußen nutzen.* Er
trägt Terminal-Bytes und Steuerung und sonst nichts; die Bridge ist der einzige Weg zum Proxy,
und der Daemon reicht nie ein Byte des Stroms an den Proxy weiter oder von ihm. Innen halten nur
PID 1 den Socket und die Herrscherseiten der Pseudo-Terminals: Das Kind des Agenten schließt sie
vor seinem Filter, jede weitere Shell schließt jeden Deskriptor über 2, alle tragen `CLOEXEC`,
und PID 1 ist nicht dumpbar. Im Gast lauscht der Shim auf genau einem vsock-Port und nimmt genau
zwei Verbindungen vom Host an, zuerst den Bericht, dann diesen Strom, und schließt danach den
Lauscher; der Gast öffnet für keinen von beiden eine Verbindung. (2) *Einen eigenen vsock- oder
Unix-Socket öffnen.* Der Filter des Agenten verweigert `AF_VSOCK` und `AF_UNIX` mit `EPERM`,
ebenso der Filter, den jede weitere Shell vor `exec` setzt und belegt, bevor sie startet; ESC-1
prüft das im Strom-Modus. (3) *Dem Daemon feindliche Rahmen schicken* (Root im Gast kann
beliebiges auf den Strom schreiben). Jeder Rahmen hat einen Kopf von neun Bytes mit einer Länge
von höchstens 16 KiB; ein längerer Rahmen wird weggezählt, ohne ihn zu behalten, ein Rahmen
unbekannter Art ebenso übersprungen, ein Rahmen falscher Form oder der Gegenrichtung verworfen,
und eine Antwort für eine Kennung, die niemand geöffnet hat, ändert nichts. Der Leser hält
höchstens einen Rahmen. Das Fuzz-Target `control_stream_reader` prüft diesen Leser jede Nacht,
und sein Regressionstest scheitert, wenn die Längenprüfung fehlt. (4) *Den Daemon überleben.* Das
Ende des Stroms ist eine Lebensleine: PID 1 schickt dem Agenten `SIGTERM`, beendet jede weitere
Shell und nach der Frist jeden Prozess. PID 1 schreibt nie selbst in ein Terminal und schickt das
Signal vor jeder Meldung, also kann ein volles Terminal es nicht aufhalten; nach dem Ende liest es
jedes Terminal leer und behält nichts. Im Gast ist das Tiefenverteidigung; der Host beendet die VM
selbst (K-33, HUM-444). *Restrisiko:* Root im Gast kann den Strom beenden oder anhalten. Ein Gast, der
nicht mehr liest, lässt den nächsten Rahmen des Daemons seine Frist von zehn Sekunden verfehlen,
und der Daemon schließt dann den Strom: Die Session endet, kein Faden des Daemons wartet länger.
Ein Daemon, der nicht mehr liest, stoppt das Lesen der Terminals im Shim, nicht das der
Steuerrahmen. Eingaben für ein Terminal, dessen Prozess nicht liest, werden verworfen, sobald seine
Schlange 64 KiB hält, und Ausgaben einer weiteren Shell, deren Leser stockt, verwirft der Daemon
über 256 Stücke hinaus; beides wird gezählt, keins hält einen anderen Kanal auf.

### K-30 Der Monitor der virtuellen Maschine der Kernel-Stufe

Die Kernel-Stufe (ADR-0021, HUM-444; nicht zugelassen bis HUM-449) setzt den Agenten in eine
QEMU-micro-VM mit eigenem Kernel. Der Emulator ist der Code zwischen einem Gast, den Angreifer (b)
ganz besitzen kann, und dem Host. Er bekommt kein Netzgerät (`-nic none`, kein `-netdev`), keine
Konsole, keinen Monitor, keine Anzeige und kein QMP, also keinen Kanal, über den ein Gast sprechen
könnte, außer den drei vhost-user-Sockets seiner Geräte. Er läuft unter seinem vollen `-sandbox`
(keine veralteten Systemaufrufe, kein Rechtewechsel, kein `fork`/`exec`, keine Steuerung von
Ressourcen), in einem bubblewrap-Gefängnis ohne Fähigkeiten, das nur `/usr`, `/dev/kvm`, die eine
Kernel-Datei und, nur lesbar, die drei Verzeichnisse seiner vhost-user-Sockets hält (nie die Tür,
den Übergabe-Socket oder die Dateien des Gasts), und in einer vorübergehenden Unit des Nutzers mit
`PrivateNetwork`,
`PrivateUsers`, `NoNewPrivileges`, ohne Fähigkeit und mit nur lesbarem Dateisystem. Nach dem Start
prüft der Daemon ihn vom Host aus: Seine Deskriptoren dürfen nur KVM, Ereignis- und
Signal-Deskriptoren, `io_uring`, sein Speicher, `/dev/null`, Pipes und Sockets sein; seine eigene
Startzeile darf keine Netzoption und keine Konsole tragen; ein Fund ist `SANDBOX_047` und beendet
die Sitzung. Hat der Gast selbst schon abgelehnt, ist seine Diagnose die angezeigte, und die Funde
des Hosts stehen im Befund; abgewiesen wird die Sitzung so oder so (HUM-470). Der Agent im Gast wartet an seinem Tor vor dem `exec` auf das `ADMIT` des Daemons,
das erst nach den Zeilen des Gasts und dieser Host-Prüfung geht; ein Fund beendet die Sitzung also,
bevor der Agent je läuft. *Restrisiko:* ein Fehler in KVM oder im Gerätecode des Emulators, der einen Gast Code im
Emulator ausführen lässt; Gefängnis und Unit begrenzen dann, was dieser Code erreicht, und Netz hat
er weiterhin nicht.

### K-31 Die vsock-Tür der Kernel-Stufe

Der einzige Weg vom Gast zum Host ist vsock: ein Gerät je Sitzung, bedient von einem eigenen
Prozess `vhost-device-vsock` mit eigener Gruppe (zwei Gäste in einem Prozess könnten einander
erreichen, HUM-442 Q9). Eine Verbindung des Gasts an CID 2 und Port `P` wird eine Verbindung an den
Unix-Socket `<uds>_P` im Laufzeitverzeichnis der Sitzung. Der Daemon bindet genau einen solchen
Socket, `<uds>_5000`, und reicht jede Verbindung an den Proxy-Socket der Sitzung weiter; für jeden
anderen Port gibt es keinen Lauscher, und die Verbindung des Gasts läuft in ein Timeout (Q2).
`--forward-cid` ist nie gesetzt. Die Seite des Hosts öffnet selbst zwei Verbindungen in den Gast
(`CONNECT` am Berichts-Port des Shims): den Bericht und den Steuerkanal (K-29). Ein gestelltes
`CONNECT` gibt sie nie auf, um neu zu fragen; der Gast, der genau zwei Verbindungen in ihrer
Reihenfolge nimmt, kann so keine aufgegebene für den Bericht halten (HUM-470). Die Host-Prüfung
lehnt jeden Socket im Laufzeitverzeichnis außer den erklärten ab, und mehr als einen
vsock-Geräteprozess oder ein zweites Gerät. *Restrisiko:* ein Fehler im Prozess des vsock-Geräts;
er läuft eingesperrt und ohne Netz wie die anderen Hilfsprozesse.

### K-32 Der Gastkernel

Der Agent im Gast ist kein Root: Vor seinem Filter gibt er jede Fähigkeit ab, die Begrenzungsmenge
eingeschlossen, und läuft als der Nutzer, den bwrap in seine Sandbox abbildet; er kann also nicht
in PID 1 schreiben (`/proc/1/mem`) und kein Modul laden (HUM-445). Das `/sys` des Gasts ist ein
leeres, nur lesbares tmpfs, wie unter bubblewrap, das kein sysfs einhängt. Ein Fehler im Kernel
kann den Agenten trotzdem zu Root machen, und Root beherrscht den Gastkernel. Keine der drei
Garantien ruht auf ihm: Ein Netzgerät gibt es nicht, die Tür entscheidet der Host, und der Emulator kann nach
dem Start kein Gerät dazubekommen. Bevor der Agent läuft, leert der Shim als PID 1
`kernel.modprobe` (kein Modul lädt auf Anforderung), lädt genau die vsock-Module des Plans und
sperrt das Laden von Modulen (`kernel.modules_disabled`); die Wurzel des Gasts ist eine nur lesbare
Freigabe, die weder das Laufzeitverzeichnis noch das `/proc` des Gefängnisses hält. Die
`CHECK`-Zeilen des Shims aus dem Gast sind Tiefenverteidigung, und ein Gast, der ohne sie endet, ist
eine gescheiterte Prüfung. Die Escape-Suite misst den Gast als Root ohne seccomp-Filter
(`tests/escape/guest-root.sh`, HUM-445): nur `lo`, kein Weg nach außen, die Tür des Hosts das
einzige vsock-Ziel, kein zweiter Gast erreichbar, kein Modul. *Restrisiko:* Root im Gast kann die
Zeilen des Gasts fälschen; dann hält die Host-Prüfung.

### K-33 Die Hilfsprozesse einer Sitzung

Eine Sitzung der Kernel-Stufe ist eine vorübergehende Unit des Nutzers mit fünf eigenen
Prozessen: dem ersten Prozess der Unit (der Shim mit `--fds-then-exec`, dann ein festes
`sh`-Skript), dem Dateisystemdienst der Wurzel des Gasts, dem Dateisystemdienst für `/work`, dem
vsock-Gerät und dem Emulator. Jeder Hilfsprozess läuft in einem eigenen bubblewrap-Gefängnis mit
allen Namensräumen getrennt und ohne jede Fähigkeit, sieht nur `/usr` des Hosts, was er
ausliefert, und sein eigenes Socket-Verzeichnis im Laufzeitverzeichnis (kein Gefängnis bindet das
Laufzeitverzeichnis selbst), und die Unit selbst hat kein Netz, keine Fähigkeit und ein nur lesbares Dateisystem
außer dem Laufzeitverzeichnis der Sitzung und dem Projektordner. Die Unit ist kein Kind des Daemons
und erbt von ihm nichts außer drei Deskriptoren über einen Übergabe-Socket, der eine Verbindung mit
einem Einmal-Zeichen annimmt: den lauschenden Socket des Wurzeldienstes, den geprüften
Projektordner und das Leseende der Lebensleine. Endet der Daemon, auch durch `SIGKILL`, sieht das
Skript das Ende der Lebensleine und endet, und der Dienstverwalter beendet binnen Sekunden jeden
Prozess der Unit, ohne den Gast; ein Daemon, der als Dienst läuft, bindet die Unit zusätzlich an
seine eigene (`BindsTo`). *Restrisiko:* Ein übernommener Daemon kann solche Units starten (wie unter
Podman, siehe Vertrauensgrenzen); ein Fehler in einem Dateisystemdienst erreicht, was er ausliefert,
die nur lesbare Wurzel des Gasts oder den Projektordner.

## Annahmen

| ID | Annahme | Wie man sie prüft | Wenn sie bricht |
|---|---|---|---|
| A-1 | `bubblewrap` 0.8 oder neuer ist installiert und darf unprivilegierte User-Namespaces anlegen | `bwrap --version`; `sysctl kernel.unprivileged_userns_clone` oder `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` | keine Session startet; kein schwächerer Ersatz, sondern eine Diagnose mit dem Befehl zur Abhilfe |
| A-2 | der Kernel ist 5.4 oder neuer (das Minimum, das `humanitl doctor` prüft, getestet ab 5.10) mit seccomp-BPF | `uname -r`; `grep Seccomp /proc/self/status` | das Kind des Shims kann den Filter nicht installieren und hält vor dem `exec` an (`SANDBOX_016`, oder `SANDBOX_013` ohne Bericht) |
| A-3 | der Mensch liest, was er freigibt | technisch nicht prüfbar; das Fenster macht das Lesen billig: Funde zuerst, vollständiger Body, Ziel als Identität | Humanitl ist dann ein Recorder, kein Schutz; der Verlauf zeigt trotzdem, was freigegeben wurde |
| A-4 | der LLM-Host steht unter der Kontrolle des Nutzers | `humanitl config get llm.endpoint`; Zeile 4 des Isolationsbereichs | der Modellkanal ist voller Zugriff auf alles, was der Agent dem Modell schickt |
| A-5 | der Rechner ist nicht kompromittiert | der Download gegen `SHA256SUMS` seines Releases ([verify-download](guides/verify-download.md); signiert wird ab 1.0.0), `systemctl --user status humanitld` | jede Aussage hier ist hinfällig |
| A-6 | die Sandbox-Profile sind die mitgelieferten oder absichtlich geänderte | `humanitl sandbox argv` | ein zusätzlicher Socket lässt die Isolationsprüfung scheitern; ein zusätzlicher Datei-Mount ist nur in der bwrap-Zeile sichtbar |

## Außerhalb des Umfangs

Gegen diese Angreifer gibt es keine Abwehr, und Humanitl behauptet das auch nicht:

- **Kompromittierung des Rechners.** Root auf dem Rechner liest den CA-Schlüssel, den
  Schlüsselbund und den Speicher des Daemons und kann eine CA in den Systemspeicher legen, dem der
  Daemon gegenüber Zielen vertraut (HUM-257). Kernel-Exploits, die Namespaces oder seccomp brechen,
  gehören ebenfalls hierher; die Gegenmaßnahme ist die Wahl viel genutzter Komponenten und
  rechtzeitige Kernel-Updates.
- **Prozesse desselben Kontos außerhalb der Sandbox**, die die Einrichtung des Daemons ändern:
  seine Umgebung (zum Beispiel `SSL_CERT_FILE`, `SSL_CERT_DIR`) oder `config.toml`. Das ist nicht
  Angreifer (d), der Aufzeichnungen ändert; wer den Daemon konfiguriert, gilt als
  vertrauenswürdig.
- **Physischer Zugriff**, eine nicht gesperrte Sitzung, Cold-Boot-Angriffe, Speicherabbilder.
- **Ein böswilliger Nutzer.** Wer eine Anfrage freigibt, gibt sie frei. Humanitl liefert die
  Grundlage für die Entscheidung; es trifft sie nicht und setzt sich nicht über sie hinweg. Manipulierte
  Humanitl-Binaries gehören ebenfalls hierher. Releases vor 1.0.0 sind nicht signiert: `SHA256SUMS`
  erkennt einen beschädigten Download, kein manipuliertes Release; signierte Releases kommen mit
  1.0.0 (HUM-060). Updates aus der App prüfen ein signiertes Manifest (K-25).
- **Mikroarchitektur- und Zeit-Seitenkanäle**, elektromagnetische Abstrahlung.
- **Denial of Service** gegen den Agenten oder den Daemon.

## Restrisiken

- **Nicht aufgezeichneter Verkehr während eines Ausfalls der Aufzeichnung** (K-16). Der Verkehr
  geht weiter, während der Recorder nicht schreiben kann; das Fenster sagt das, aber was in dieser
  Zeit über Regeln und die Durchleitung hinausgeht, hat keine Aufzeichnung.
- **Die Audit-Kette gegenüber demselben Nutzer** (K-14). Ein Prozess desselben Kontos hat den
  HMAC-Schlüssel und die Datenbank und kann die Geschichte umschreiben; nur eine externe
  Verankerung (nach dem MVP) würde das ändern.

- **Der LLM-Host ist geteiltes Vertrauen.** Alles, was der Agent dem Modell zeigt, sieht der
  Modell-Host. Ist der Host geteilt, fremd oder kompromittiert, hilft keine Sandbox; der Kanal ist
  absichtlich offen. Wer keinen eigenen Rechner dafür hat, schaltet die Durchleitung ab und lässt
  die Inferenz anhalten.
- **Ausleitung über `/work`.** Alle drei Angreifer können ins Projekt schreiben. Humanitl zeigt am
  Ende eines Laufs die berührten Dateien, Symlinks und Secrets, aber was ein Mensch später
  committet, pusht oder per Mail verschickt, geht ohne Humanitl hinaus.
- **Der Mensch als Klassifikator.** Ein base64-Body an `api.github.com` sieht legitim aus.
  Humanitl stellt Funde voran, gruppiert nach Host, entfernt Rauschen mit Regeln, fragt nach, bevor
  ganze Gruppen freigegeben werden, und lässt eine Zeitüberschreitung in `block` enden. Die
  Aufmerksamkeit des Menschen kann es nicht lesen.

## Änderungsgeschichte

| Datum | Änderung | Grund |
|---|---|---|
| 2026-09-02 | Erste Fassung: vier Angreifer, vierzehn Kanäle, sechs Annahmen | HUM-007, Sicherheits-Review vom selben Tag |
| 2026-09-04 | K-15 ergänzt: Eine rote Isolationsprüfung hält die Session an, statt sie zu verhindern | HUM-041 |
| 2026-09-23 | K-02: Pfadmuster von `allow`/`redact` passen nie auf `..`-Segmente; `block`/`ask` prüfen auch den aufgelösten Pfad | HUM-204 |
| 2026-09-23 | K-04, K-06: Der Shim ist PID 1, gefiltert und nicht dumpbar; `pidfd_getfd` in der Grundsperre | HUM-203 |
| 2026-09-23 | K-13 sichtbar: verweigerte `socket()`-Aufrufe über einen seccomp-Listener im Elternprozess des Shims gezählt | HUM-138 |
| 2026-09-24 | Harte Sperre: Ein durch Prüfsumme bestätigtes Secret wird zum Bearbeiten angehalten; `Allow` mit `HOLD_004` verweigert | HUM-159 |
| 2026-10-06 | K-02: der Zugangsschlüssel des Anbieters im Schlüsselbund des Systems, für den Agenten nur als Datei in der Sandbox, in der Aufzeichnung verdeckt | HUM-140 |
| 2026-10-19 | `stream: true` als erklärte Ausnahme vom Puffern, nie unter der harten Sperre | HUM-057 |
| 2026-10-20 | Der Daemon vertraut gegenüber Zielen dem Zertifikatsspeicher des Systems; abschaltbar mit `tls.trust_system_store` | HUM-257 |
| 2026-10-20 | K-02: erzeugte Endpunkte der Durchleitung passen genau, nicht als Präfixe | HUM-261 |
| 2026-10-21 | K-01: Projekt und zusätzliche Mounts über Deskriptoren eingebunden | HUM-263 |
| 2026-10-21 | Ausfall der Aufzeichnung erreicht jeden Client und das Fenster | HUM-250 |
| 2026-10-21 | Ins Englische übersetzt und auf den Stand des Codes für 0.1 gebracht | HUM-059 |
| 2026-10-22 | K-17: Agentenprogramm aus einem Verzeichnis des Nutzers als einzelne Datei nur lesbar eingebunden | HUM-278 |
| 2026-10-23 | K-02: mehrere gespeicherte Modellserver, je ein Eintrag im Schlüsselbund; nur der aktive Ursprung bekommt einen Schlüssel und die Durchleitung | HUM-289 |
| 2026-10-24 | K-06: Der erste Prozess der Session übernimmt das Pseudo-Terminal des Daemons als steuerndes Terminal, `--new-session` bleibt; K-17: Der Befehl eines Aufrufers bekommt den konfigurierten Agenten daneben eingebunden, nie sich selbst | HUM-280 |
| 2026-10-29 | B1: Die eigenen Erlaubnisabfragen des Agenten sind keine Grenze; die von OpenCode für `webfetch` und `bash` sind aus | HUM-308 |
| 2026-10-29 | K-18: weitere Shells neben dem Agenten über ein Socket-Paar, das nur der Daemon und PID 1 halten, innen geforkt mit dem Filter des Agenten obendrauf | HUM-299 |
| 2026-10-29 | B1: Eine global installierte pi-Erweiterung (`pi-web-access`) läuft im Agenten mit all seinen Beschränkungen; ihre Anfragen gehen durch den Proxy und werden dort angehalten, wenn keine Regel über sie entscheidet; `agent.pi.web_access` für Projekt-Profile gesperrt | HUM-309 |
| 2026-10-30 | K-19 bis K-23: macOS mit Seatbelt; gemeinsames Loopback, kein PID-Namespace, Pfade statt Deskriptoren, eine veraltete Schnittstelle, lokale Seitenkanäle auf dem Rechner | HUM-323, HUM-327, HUM-331 |

| 2026-10-30 | K-24: die Wegwerf-Sandbox, ein Overlay über dem nur lesbaren Projekt mit der oberen Schicht im RAM und einer Größengrenze; Änderungen erreichen das Projekt nur über eine geprüfte Übernahme | HUM-350 |
| 2026-10-30 | K-02: unter macOS liegt der Zugangsschlüssel des Anbieters im Anmelde-Schlüsselbund über `/usr/bin/security`, über `stdin` und zurückgelesen; dieselbe Klasse wie der Secret Service: lesbar für jeden Prozess des Kontos außerhalb der Sandbox, außer Reichweite des Agenten | HUM-349 |
| 2026-10-30 | K-24: das Überschreiben eines Konflikts wird je Änderung bestätigt (`overwrite_ids`); eine Bestätigung für eine Datei deckt nie eine andere ab | HUM-351 |
| 2026-10-31 | K-02: die Grenze ist der gespeicherte Server, den die Session aufgelöst hat; `llm.server` aus Lauf, Projekt oder Profil wählt unter den gespeicherten Servern des Nutzers, legt nie einen an; ein unbekannter Name lehnt den Start ab | HUM-400 |
| 2026-11-12 | K-06: die Lebensleine des Shims (`--lifeline-fd`) als Gegenstück zu `--die-with-parent` für ein Backend, das die Sandbox vom Daemon abkoppelt; die Umgebung des Agenten über einen versiegelten memfd (`--env-fd`) statt `argv`; der Agent erbt keinen von beiden; K-15 unverändert | HUM-430 |
| 2026-11-12 | K-27 neu: die Container-Engine unter dem Podman-Backend, gemeinsame Stufe, nicht zugelassen bis HUM-448 | HUM-433 |
| 2026-11-12 | K-27: ESC-1 und ESC-2 laufen gegen Podman im CI-Job `escape-tests-podman`, mit einer gepflanzten feindlichen Engine-Konfiguration des Nutzers (`mounts.conf`, Volume, Hook), die die Sandbox nicht erreichen darf | HUM-435 |
| 2026-11-12 | K-24: keine Wegwerf-Sandbox unter Podman; das Overlay-Volume der Engine wurde gemessen (P10) und wird nicht verwendet, weil sich seine untere Schicht nicht an den geprüften Ordner binden lässt; `disposable` lehnt mit `SANDBOX_030` ab, `auto` läuft direkt mit `SANDBOX_033` | HUM-438 |
| 2026-11-12 | K-01, K-05, K-27: Der Shim vergleicht unter jedem Backend die Mounttabelle mit dem Plan und Gerät und Inode jedes geprüften Ordners, bevor der Agent startet (`SANDBOX_049`, `SANDBOX_042`); unter Podman wird ein getauschter Ordner erkannt und abgelehnt statt verhindert; K-27 nennt die Konfigurationsquellen von Podman | HUM-437 |
| 2026-11-13 | K-28 neu: ein Abbild als Wurzel der Sandbox unter Podman, mit Digest festgelegt und vor jedem Start geprüft (`SANDBOX_043`), gezogen nur auf Wunsch und direkt, nicht durch den Proxy; ESC-2 prüft die CA nach Gerät und Inode | HUM-439 |
| 2026-11-13 | Vertrauensgrenzen: Unter seiner gehärteten Unit startet der Daemon Podman als vorübergehende Units `humanitl-*` des Dienstverwalters des Nutzers, statt seine Härtung zu lockern; ein übernommener Daemon gewinnt „eine vorübergehende Unit des Nutzers starten". K-27: die Deskriptoren des Shims über einen Socket der Übergabe mit genau einer Verbindung statt `--preserve-fds` | HUM-434 |
| 2026-11-13 | K-28: Zugangsdaten zur Registry beim Ziehen nur aus der eigenen Datei des Adapters, als private Kopie unter `--authfile`; Credential-Helfer in dieser Datei oder in einer Ergänzung zur `registries.conf` abgelehnt (`SANDBOX_043`) | HUM-456 |
| 2026-11-13 | K-27 `mitigated`: die gemeinsame Stufe von Podman (`podman/crun`) zugelassen, nachdem `escape-tests-podman` (ESC-1, ESC-2, ESC-5) mit HUM-434, HUM-436, HUM-437 und HUM-438 auf `main` grün war; gewählt nur von einem Menschen, nie von `auto`; die Kernel-Stufe lehnt jeder Start bis HUM-449 mit `SANDBOX_048` ab | HUM-448 |
| 2026-11-14 | K-29 neu: der Steuerkanal der Terminals, der Strom-Modus der Kernel-Stufe und von bwrap auf Verlangen; K-18 im Strom-Modus; weitere Shells belegen zusätzlich ein verweigertes `AF_VSOCK` | HUM-443 |
| 2026-11-14 | K-30 bis K-33 neu: Monitor, vsock-Tür, Gastkernel und Hilfsprozesse der Kernel-Stufe; eine gehärtete Unit je Sitzung, vier Gefängnisse, der geprüfte Projektordner als Deskriptor, eine Host-Prüfung des Monitors (`SANDBOX_047`), bevor der Agent zugelassen wird, jedes Gefängnis bindet nur sein eigenes Socket-Verzeichnis, das Ende des Daemons beendet die Unit; nicht zugelassen bis HUM-449 | HUM-444 |
| 2026-11-14 | K-30, K-31: Hat der Gast schon abgelehnt, wird seine Diagnose angezeigt, die Funde der Host-Prüfung stehen im Befund, abgewiesen wird die Sitzung so oder so; der Host gibt ein gestelltes `CONNECT` in den Gast nie auf | HUM-470 |
| 2026-11-14 | K-32: die tmpfs des Profils (`/tmp`, `/var/tmp`, das Heimatverzeichnis des Agenten) und `/run` sind im Gast beschreibbare tmpfs, `/run/humanitl` nur lesbar darüber, eingehängt vom Shim, in der erwarteten Mounttabelle und in der Socket-Suche; `mounts.extra_rw` auf der Kernel-Stufe abgelehnt (`SANDBOX_051`) statt nur lesbar eingehängt | HUM-466 |

Jede sicherheitsrelevante Änderung am Shim, an der Mount-Erlaubnisliste, am Filter oder an der
Durchleitung bekommt hier eine Zeile. Schwachstellen werden gemeldet, wie in
[`SECURITY.md`](SECURITY.md#10-reporting-a-vulnerability) beschrieben.
