# Humanitl — Bedrohungsmodell

> Stand: 2026-09-02 (Entwurf, HUM-007). Verbindliche Kurzfassung: `BACKLOG.md` Abschnitt 4.
> Die Mechanismen und ihre Prüfbefehle stehen in [`SECURITY.md`](SECURITY.md); dieses Dokument
> beschreibt, gegen wen sie wirken und wo sie nicht wirken.

Dieses Dokument ist für zwei Leser geschrieben. Wer kein Sicherheitsfachmann ist, soll nach
Abschnitt 1 bis 3 wissen, was Humanitl verhindert und was nicht. Wer ein Review macht, soll
jede Behauptung nachprüfen können: zu jedem Kanal steht der Mechanismus, die Datei, in der er
implementiert wird, und der Test, der ihn beobachtet.

---

## 1. Schutzziel

Humanitl schützt genau zwei Dinge.

**Vertraulichkeit von Projektdaten und Zugangsdaten.** Nichts aus dem Projektverzeichnis, aus der
Umgebung des Agenten oder aus dem Gedächtnis des Agenten verlässt die Maschine über das Netz, ohne
dass eine Regel oder ein Mensch die konkrete Anfrage freigegeben hat. Ohne passende Regel sieht ein
Mensch sie vorher: Methode, Ziel, Header, Parameter und der vollständige Anfrage-Body, nicht nur der
Hostname. Eine Ausnahme ist erklärt und sichtbar: der Passthrough zum lokalen Sprachmodell wird
gestreamt, vollständig protokolliert und auf Findings geprüft, aber nicht angehalten (Abschnitt 3,
Kanal LLM). Ebenso offen bleibt das Projektverzeichnis `/work`.

**Integrität der Entscheidungshistorie.** Was entschieden wurde, wann und mit welcher Begründung,
lässt sich später nicht unbemerkt umschreiben. Das ist die Grundlage dafür, gegenüber einem
Kunden oder einer Aufsichtsbehörde zu belegen, was ein Agent tatsächlich nach draußen gegeben
hat.

**Ausdrücklich kein Schutzziel: Verfügbarkeit des Agenten.** Wenn ein Mechanismus versagt, hört
der Agent auf zu arbeiten. Ein Timeout endet in `block`, nie in `allow`. Ein fehlgeschlagener
Isolations-Check verhindert den Start, statt ihn mit einer Warnung zuzulassen. Ein Agent, der
steht, ist ein akzeptabler Ausgang; ein Agent, der ungesehen sendet, nicht.

---

## 2. Angreifer

### 2.1 Wer im Modell ist

Der Angreifer ist in allen Fällen **Code, der als der Agent läuft, mit unterwanderter Absicht**.
Er hat die Rechte des Agenten: er kann in `/work` lesen und schreiben, Prozesse starten und den
Proxy ansprechen. Er hat nicht die Rechte des Kernels und nicht die des Hosts.

| ID | Angreifer | Wie er hineinkommt | Was er kann | Was er nicht kann |
|---|---|---|---|---|
| **a** | **Prompt Injection** über geladene Inhalte | Ein README, ein Issue-Text, eine Webseite, eine Fehlermeldung eines Paket-Servers enthält Anweisungen, die das Modell als Auftrag liest | Werkzeugaufrufe des Agenten steuern: Dateien lesen, Anfragen formulieren, Text ans Modell schicken | Keine eigenen Binaries mitbringen; er arbeitet mit den Werkzeugen, die der Agent hat |
| **b** | **Bösartige Abhängigkeit** | Ein Paket, das der Agent installiert oder ausführt (`npm install`, `pip install`, ein Build-Skript) | Beliebiger Code als der Agent-Benutzer in der Sandbox: Syscalls, eigene Netzwerkversuche, Lesen aller sichtbaren Dateien | Nichts, was der Kernel ihm verweigert; keine neuen Mounts, keine Capabilities |
| **c** | **Kompromittiertes Modell** | Das lokale Modell selbst ist manipuliert (vergiftete Gewichte, untergeschobener Endpunkt) | Alles von (a), und es sieht zusätzlich jeden Prompt, also potenziell den gesamten Quelltext, den der Agent ihm zeigt | Nichts an der Sandbox ändern; es kann nur den Agenten steuern |
| **d** | **Lokaler Leser der Protokolle** | Ein anderer Prozess oder ein anderer Mensch mit Dateizugriff auf `$XDG_DATA_HOME/humanitl` | Aufzeichnungen lesen, Einträge löschen oder verändern, die Datei kürzen | Die Kette unbemerkt neu bauen, solange er den HMAC-Schlüssel aus dem Keyring nicht hat |

Angreifer (a) und (b) unterscheiden sich in der Mächtigkeit erheblich: (a) muss durch den Agenten
hindurch arbeiten, (b) führt Code aus. Alle Aussagen dieses Dokuments gelten für (b), außer sie
sagen ausdrücklich etwas anderes; damit gelten sie erst recht für (a).

Angreifer (c) ist der unangenehmste, weil der Modell-Kanal in Humanitl offen ist. Wer das Modell
kontrolliert, sieht alles, was der Agent ihm zeigt. Humanitl reduziert diesen Angreifer auf
„der Nutzer hat sich entschieden, diesem Host zu vertrauen" und macht die Entscheidung sichtbar
(Abschnitt 4, K-02).

### 2.2 Ausdrücklich außerhalb des Modells

Diese Angreifer werden nicht abgewehrt, und Humanitl behauptet nicht, es zu tun.

- **Root auf dem Host.** Wer root ist, liest den CA-Schlüssel, den Keyring und den Speicher des
  Daemons. Es gibt keine Verteidigung darunter.
- **Kernel-Exploits.** Ein Fehler in der Namespace- oder seccomp-Implementierung hebt die
  Garantien auf. Die Gegenmaßnahme ist die Wahl bewährter, weit benutzter Bausteine
  (`bubblewrap`, seccomp-BPF) statt eigener Konstruktionen, plus zeitnahe Kernel-Updates.
- **Physischer Zugriff**, laufende Sitzung am entsperrten Rechner, Cold-Boot-Angriffe.
- **Manipulierte Humanitl-Binaries.** Wer das Paket austauscht, bevor es startet, kontrolliert
  alles. Gegenmaßnahme ist Paketsignatur (HUM-060), nicht dieses Modell.
- **Der Mensch selbst.** Wer eine Anfrage freigibt, gibt sie frei. Humanitl macht die Entscheidung
  informiert, es trifft sie nicht.
- **Mikroarchitektur-Seitenkanäle** (Spectre-Klasse), Timing-Angriffe auf den Host, elektro-
  magnetische Abstrahlung.
- **Denial of Service** gegen den Agenten oder den Daemon (siehe Abschnitt 1, kein Schutzziel).

---

## 3. Angriffsflächen im Überblick

Ein „•" bedeutet: dieser Angreifer kann diesen Kanal nutzen. Die Kanal-IDs sind dieselben wie in
Abschnitt 4. Die Spalte „Escape-Test" nennt den automatisierten Test, der den Kanal beobachtet
(`tests/escape/`, Definition in `BACKLOG.md` 4.5).

| ID | Kanal | a | b | c | d | Gestoppt durch | Restrisiko | Escape-Test |
|---|---|:-:|:-:|:-:|:-:|---|---|---|
| K-01 | Projektverzeichnis `/work` | • | • | • | | nichts — der Kanal ist gewollt offen | vollständig: was der Agent schreibt, steht auf der Platte | ESC-5 |
| K-02 | LLM-Passthrough | • | • | • | | enge Regel (Host, Port, Schema, Pfadpräfixe der Inferenz, POST und GET), vollständige Aufzeichnung | vollständig: der Modell-Host sieht jeden Prompt | — |
| K-03 | Socket-Verzeichnis des Proxys | | • | | | Bind der Socket-**Datei**, nie des Verzeichnisses | Fehler im Proxy selbst | ESC-2 |
| K-04 | Von seccomp erlaubte TCP-Sockets | | • | | | leeres Netz-Namespace, leere Routing-Tabelle | Loopback-Dienste innerhalb der Sandbox | ESC-1 |
| K-05 | Unix-Sockets des Hosts (X11, Wayland, D-Bus, Docker) | | • | | | Mount-Allowlist im Profil | eine falsche Zeile im Profil öffnet ihn | ESC-2 |
| K-06 | PID- und IPC-Namespace | | • | | | `--unshare-pid --unshare-ipc`, eigenes `/proc` | — | ESC-2 |
| K-07 | `/proc` und `/sys` | | • | | | eigenes `procfs`, `/sys` nicht gebunden | Kernel-Informationslecks | ESC-2 |
| K-08 | Symlinks aus `/work` heraus | | • | | | Mount-Namespace; Markierung in der Sitzungs-Zusammenfassung | der Nutzer übersieht die Markierung | ESC-5 |
| K-09 | Terminal-Escapes (OSC 52, OSC 8, Titel) | • | • | • | | Escapes im Terminal-Widget abgeschaltet | Fehler im Widget | ESC-5 |
| K-10 | DNS und Auflösungs-Zeitpunkt | • | • | • | | Auflösung erst nach Freigabe (ADR-006) | Zeitpunkt der Freigabe bleibt beobachtbar | ESC-3 |
| K-11 | Queue- und Regel-Metadaten | • | • | • | | `humanitl.internal` ist lesend, `/ask` erzeugt nie eine Regel | Entscheidungs-Orakel bleibt | ESC-4 |
| K-12 | Package-Caches | | • | | | Cache pro Projekt, nicht pro Vertrauensstufe | geteilter Cache trägt Daten zwischen Projekten | — |
| K-13 | Direkter Netzwerk-Egress | • | • | • | | Netz-Namespace + seccomp + Capability-Drop | Kernel-Fehler | ESC-1, ESC-3 |
| K-14 | Audit-Datei auf dem Host | | | | • | Hash-Kette mit HMAC | Kürzen am Ende, Angreifer mit Schlüssel | ESC-5 |
| K-15 | Fenster zwischen `exec` und `SIGKILL` | | • | | | Bericht sofort nach dem Start, `SIGKILL` ohne Gnadenfrist | offen: der Proxy kennt die Isolation der Sitzung noch nicht | — |

K-01 bis K-12 sind die zwölf Kanäle der Sicherheits-Review vom 2026-09-02. K-13 steht dabei, weil
ein Modell ohne den offensichtlichsten Kanal unvollständig wäre — er ist der einzige, der als
geschlossen gilt. K-14 steht dabei, weil sonst Angreifer (d) in keiner Zeile vorkäme. K-15 kam am
2026-09-04 mit HUM-041 dazu: Die Prüfung, die die drei Garantien belegt, läuft erst, wenn der Agent
schon gestartet ist.

---

## 4. Die Kanäle im Einzelnen

Schwere ist die Wirkung, **wenn** der Kanal genutzt wird, nicht die Wahrscheinlichkeit.
Status ist entweder **MVP** (die beschriebene Minderung ist Teil von 0.1), **später** (geplant,
mit Issue) oder **offen** (deklarierter Seitenkanal, wird nicht geschlossen, sondern gezeigt).

### K-01 Projektverzeichnis `/work`

*Was er ist.* Der Agent braucht Schreibzugriff auf das Projekt, sonst kann er nicht arbeiten. Alles,
was er dort ablegt, liegt danach auf der Platte des Nutzers: auch eine Datei, die Zugangsdaten in
Klartext enthält, oder ein Commit, der Daten in ein fremdes Repository schiebt, sobald der Mensch
das nächste Mal pusht.

*Schwere.* Hoch. Dies ist der breiteste offene Kanal.

*Minderung im MVP.* Nur ein Unterpfad wird gebunden, nie `$HOME`. `work_mode = "ro"` ist möglich
und für Analyse-Sitzungen der Vorschlag. Verzeichnisse wie `.git/hooks`, `.vscode`, `.idea` und
`.github/workflows` werden mit einem leeren `tmpfs` überdeckt, Dateien wie `.envrc`, `.env`,
`.git/config`, `.npmrc` und `Jenkinsfile` mit einer leeren, nur lesbaren Datei aus einem
versiegelten `memfd` (`--ro-bind-data`, nicht `/dev/null`: der Bind eines Gerätes auf einen
`nodev`-Mount antwortet `EACCES`). Die vollständige Liste steht in
`profiles/sandbox/default.toml`; damit läuft ein geschriebener Hook nicht beim nächsten Commit auf
dem Host. Zum Ende eines Sandbox-Laufs zeigt Humanitl eine Zusammenfassung der berührten Dateien,
markiert Symlinks mit Ziel außerhalb von `/work` und führt den Findings-Scan über den Diff, nicht
nur über die Netzwerk-Anfragen.

*Restrisiko.* Ein Agent kann Daten im Projekt umschichten, verschleiern oder in einer Datei
ablegen, die der Mensch später selbst nach außen trägt. Humanitl sieht das nur, wenn der Mensch
den Diff liest.

*Status.* Offen (deklariert). Git-Operationen host-seitig aus dem UI sind nach dem MVP vorgesehen.

*Prüfung.* `profiles/sandbox/default.toml` (`[mounts]`), Escape-Test `tests/escape/esc-5-filesystem.sh`.

### K-02 LLM-Passthrough

*Was er ist.* Der Agent muss Code an das Sprachmodell schicken, sonst gibt es kein Produkt. Diese
Anfragen werden nicht angehalten, sonst müsste der Mensch jede Inferenz bestätigen. Damit ist der
Modell-Host der zweite offene Kanal — und, anders als `/work`, ein Kanal über das Netz.

*Schwere.* Hoch. Wer den Modell-Host kontrolliert, sieht alles, was der Agent ihm zeigt.

*Minderung im MVP.* Die Passthrough-Regel ist so eng wie möglich: exakt ein Host, ein Port, ein
Schema, die Methoden `POST` und `GET` und eine Liste von Pfadpräfixen, die im Default nur die
Inferenz deckt. Aufgezählt werden dort dreizehn Endpunkte (`/v1/chat/completions`,
`/v1/completions`, `/v1/responses`, `/v1/embeddings`, `/v1/models`, `/api/chat`, `/api/generate`,
`/api/embed`, `/api/embeddings`, `/api/tags`, `/api/show`, `/api/ps`, `/api/version`); die nackten
Flächen `/api/` und `/v1/` fehlen mit Absicht, weil sie auch `POST /api/pull`, `DELETE /api/delete`,
`POST /v1/files` und `POST /v1/load_lora_adapter` deckten und einen Agenten ungefragt am Bestand
des Servers arbeiten ließen. Alles andere an denselben Host wird
normal gehalten. Der Verkehr wird vollständig aufgezeichnet und durch die Findings-Detektoren
geschickt; ein Treffer erzeugt eine Warnung (`LLM_005`), hält aber nicht an. Im Isolations-Panel
steht der Kanal als vierte, bernsteinfarbene Zeile mit dem konkreten Endpunkt.

*Restrisiko.* Vollständig. Ein Angreifer, der über Prompt Injection den Agenten steuert, kann
Projektinhalte in einen Prompt packen; der Prompt geht an den Modell-Host, ohne Rückfrage. Der
Satz „der LLM-Host ist Teil der Vertrauensbasis" ist keine Floskel, sondern die Bedingung, unter
der das restliche Modell gilt.

Dazu kommt der Name. Steht in `llm.endpoint` ein DNS-Name, entscheidet der Resolver bei jeder
Anfrage neu, wohin die Durchreiche führt. Wer diesen einen Namen kontrolliert, lenkt sie zwischen
zwei Anfragen auf einen anderen Rechner — und weil die Regel `allow_private` trägt, ist auch der
Router oder `169.254.169.254` ein gültiges Ziel; die Prüfung auf private Adressen ist für genau
diese Regel ausgeschaltet. Das ist DNS-Rebinding an der einen Stelle, an der ADR-0006 es nicht
verhindern kann, weil dort niemand gefragt wird. Abhilfe: eine IP-Adresse statt eines Namens, oder
ein fester Eintrag unter `resolver.overrides`, der vor jeder Abfrage greift.

*Status.* Offen (deklariert). Findings-basiertes Halten und Ratenbegrenzung sind nach dem MVP
vorgesehen.

*Prüfung.* Regel im UI unter „Regeln", `humanitl rules list`, Zeile 4 des Isolations-Panels.

### K-03 Socket-Verzeichnis des Proxys

*Was er ist.* Die eine Tür ist ein Unix-Socket. Würde Humanitl das Verzeichnis binden, in dem der
Socket liegt, dann wäre jede Datei, die dort später entsteht, ebenfalls in der Sandbox sichtbar —
und der Agent könnte dort selbst Sockets anlegen, über die zwei Sandbox-Prozesse an der
Aufzeichnung vorbei sprechen.

*Schwere.* Mittel.

*Minderung im MVP.* Gebunden wird die Socket-**Datei**, nicht ihr Verzeichnis. Auf dem Host liegt
der Socket in einem eigenen Verzeichnis mit Modus 0700, getrennt vom gRPC-Socket des Daemons;
letzterer ist in der Sandbox nicht vorhanden. Der Isolations-Check zählt die Sockets in der
Sandbox und besteht nur bei genau einem erwarteten Pfad.

*Restrisiko.* Fehler im Proxy selbst. Wer den Proxy-Prozess übernimmt, hat den Netzzugang des
Hosts — deshalb ist der Daemon in Rust geschrieben und systemd-seitig gehärtet.

*Status.* MVP.

*Prüfung.* `find / -xdev -type s` in der Sandbox (ESC-2), `humanitl sandbox check`.

### K-04 Von seccomp erlaubte TCP-Sockets („die seccomp-Lücke")

*Was er ist.* Der Filter verbietet nicht alle Sockets. Er **muss** `socket(AF_INET|AF_INET6,
SOCK_STREAM)` erlauben, weil genau darüber der Agent den Proxy auf `127.0.0.1:3128` erreicht. Die
dritte Garantie ist also nicht „keine Sockets", sondern „keine anderen als diese".

*Schwere.* Niedrig, solange das Netz-Namespace leer bleibt — und mittel, sobald jemand die
Reihenfolge im Shim durcheinanderbringt.

*Minderung im MVP.* Die Sicherheit dieses erlaubten Sockets trägt nicht der Filter, sondern das
Namespace: darin existiert nur `lo`, die Routing-Tabelle ist leer, alle Capabilities sind
gedroppt (`--cap-drop ALL`), also gibt es keine Adresse, zu der eine TCP-Verbindung aufgebaut
werden könnte, außer der Loopback-Adresse. Ein `connect()` auf eine LAN-Adresse endet in
`ENETUNREACH`. Zusätzlich sperrt der Filter `ptrace`, `io_uring_*`, `process_vm_*`, `keyctl`,
`add_key`, `request_key`, die Standard-Härtung `kexec_load`, `kexec_file_load`, `init_module`,
`finit_module`, `delete_module`, `bpf`, `perf_event_open`, `userfaultfd` und alle x32-Syscalls
und setzt `PR_SET_NO_NEW_PRIVS` vor der Anwendung.

*Restrisiko.* Zwei benannte Punkte. Erstens: Der Elternprozess des Shims, der die Brücke hält,
darf als einziger Prozess in der Sandbox `AF_UNIX` öffnen; sonst käme er nicht an den
Proxy-Socket. Er trägt seit HUM-012 selbst einen Filter mit derselben Sperrliste, um genau diese
eine Familie weiter (`SandboxSeccomp::for_bridge`), denn `TSYNC` erfasst Threads und keine
Kinder. Der Agent kann ihn nicht per `ptrace` übernehmen, aber `/proc/<pid>/mem` unterliegt der
Kernel-Prüfung `ptrace_may_access` und nicht dem Filter; bei gleicher UID und
`kernel.yama.ptrace_scope = 0` ist ein Zugriff denkbar. Gewonnen ist damit nichts: die Bridge
kennt genau einen Zielsocket, den Proxy, und der zeichnet auf. Zweitens:
ein Loopback-Dienst, den der Agent selbst in der Sandbox startet, ist für ihn erreichbar. Das ist
kein Egress, sondern nur Kommunikation zwischen zwei Prozessen, die ohnehin beide dem Angreifer
gehören.

*Status.* MVP.

*Prüfung.* `daemon/bin/humanitl-shim/src/seccomp.rs` (die Filtertabelle ist dort die einzige
Quelle), `grep Seccomp /proc/self/status` in der Sandbox, ESC-1.

### K-05 Unix-Sockets des Hosts

*Was er ist.* Ein einziger falscher Mount macht die ganze Konstruktion wertlos: `/tmp/.X11-unix`
gibt Tastatur und Bildschirm, der Wayland-Socket dasselbe, der D-Bus-Socket erlaubt das Starten
von Host-Prozessen, `/var/run/docker.sock` ist root auf dem Host.

*Schwere.* Kritisch, wenn er eintritt.

*Minderung im MVP.* Das Sandbox-Profil kennt eine Allowlist, keine Denylist: gebunden wird nur,
was in der `bwrap`-Argumentliste steht, und die ist vollständig in `humanitl sandbox argv`
sichtbar (Reihenfolge verbindlich in HUM-011). Im Auszug: lesend `/usr`, `/etc/ssl`,
`/etc/alternatives`, `/etc/ld.so.cache`, die CA-Dateien, generierte `/etc/passwd`, `/etc/group`,
`/etc/hosts` und der Shim; das Projekt unter `/work` mit `tmpfs` über `.git/hooks`, `.vscode`,
`.idea` und den weiteren Verzeichnissen der Liste sowie leeren, nur lesbaren Dateien über `.envrc`,
`.git/config` und den weiteren Masken; frisch angelegt `/proc`, ein minimales
`/dev`, `tmpfs` für `/tmp` und `/dev/shm`, ein leeres `/home/agent`; dazu die Proxy-Socket-Datei.
`$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `/home`, `~/.ssh`, `~/.gitconfig`, `~/.netrc` und die
genannten Sockets sind nicht enthalten; der Argv-Builder lehnt ein Profil mit einem dieser Pfade
ab (`SANDBOX_006`), und der Isolations-Check sucht in der laufenden Sandbox zusätzlich nach ihnen.
Nach dem Filter kann der Agent ohnehin kein `AF_UNIX`-Socket mehr öffnen; der Mount-Schutz ist die
erste, der Filter die zweite Linie.

*Restrisiko.* Ein Nutzer, der ein eigenes Profil schreibt und einen dieser Pfade einträgt, hebt
den Schutz auf. Deshalb schlägt der Isolations-Check bei einem unerwarteten Socket fehl und
verhindert den Start, statt zu warnen.

*Status.* MVP.

*Prüfung.* `humanitl sandbox argv` zeigt die vollständige Kommandozeile; ESC-2 liest
`/proc/self/mountinfo` in der Sandbox.

### K-06 PID- und IPC-Namespace

*Was er ist.* Ohne eigenen PID-Namespace sieht der Agent alle Prozesse des Nutzers und liest über
`/proc/<pid>/environ` deren Umgebungsvariablen — dort stehen typischerweise Tokens. Ohne eigenen
IPC-Namespace erreicht er SysV-Shared-Memory und POSIX-Message-Queues anderer Prozesse.

*Schwere.* Hoch.

*Minderung im MVP.* `--unshare-pid --unshare-ipc --unshare-uts`, ein frisch gemountetes `/proc`
für das neue Namespace, `--new-session` (kein gemeinsames Controlling-Terminal, damit
`TIOCSTI`-Einschleusung ausscheidet) und `--die-with-parent`. PID 1 in der Sandbox ist das
Init von bwrap, darunter läuft der Shim; kein Host-Prozess ist sichtbar.

*Restrisiko.* Keines bekannt, solange bwrap die Optionen wie dokumentiert umsetzt.

*Status.* MVP.

*Prüfung.* In der Sandbox: `ls /proc` zeigt nur Sandbox-Prozesse, `cat /proc/1/environ` ist leer
oder die Umgebung von bwrap, nie die des Hosts, `hostname` liefert `sandbox` (ESC-2).

### K-07 `/proc` und `/sys`

*Was er ist.* Auch ein eigenes `procfs` verrät Kernel-Version, geladene Module, CPU-Details und
über `/proc/net/*` den Zustand des Namespace. `/sys` verrät Hardware und, wenn es gebunden wäre,
Schreibpfade in den Kernel.

*Schwere.* Niedrig bis mittel: Informationsleck, kein Egress.

*Minderung im MVP.* `/proc` ist das des neuen Namespace, `/sys` wird nicht gebunden. Der
Isolations-Check liest bewusst `/sys/class/net` und `/proc/net/dev` **in** der Sandbox, damit der
Beweis aus der Sandbox stammt und nicht vom Host behauptet wird; fehlt `/sys`, genügt
`/proc/net/dev`.

*Restrisiko.* Kernel-Informationen bleiben lesbar. Für einen Angreifer, der ohnehin Code
ausführt, ist das kein Zugewinn.

*Status.* MVP.

*Prüfung.* ESC-2, Abschnitt „mountinfo".

### K-08 Symlinks aus `/work` heraus

*Was er ist.* Der Agent legt `/work/x -> /home/nutzer/.ssh` an und liest anschließend über den
Pfad. In der Sandbox scheitert das, weil `/home` dort nicht existiert. Gefährlich wird der Symlink
**nach** der Sitzung, auf dem Host, wenn ein Werkzeug des Nutzers ihm folgt.

*Schwere.* Mittel.

*Minderung im MVP.* Der Mount-Namespace löst Symlinks nur innerhalb der Sandbox auf, ein Ausbruch
über den Pfad ist also nicht möglich. Zum Sitzungsende markiert die Zusammenfassung jeden neuen
Symlink, dessen Ziel außerhalb von `/work` liegt.

*Restrisiko.* Der Nutzer übersieht die Markierung.

*Status.* MVP (Markierung), die host-seitige Auflösung bleibt Sache des Nutzers.

*Prüfung.* ESC-5 legt `/work/x -> /home` an und erwartet den Eintrag in der Zusammenfassung.

### K-09 Terminal-Escapes

*Was er ist.* Der Mensch liest die Ausgabe des Agenten. Diese Ausgabe ist vom Angreifer
kontrolliert. Mit OSC 52 schreibt sie in die Zwischenablage des Hosts, mit OSC 8 versteckt sie
eine fremde URL hinter harmlosem Text, mit dem Titel-Escape verändert sie das Fenster.

*Schwere.* Mittel: Der Weg führt über den Menschen, nicht über den Kernel.

*Minderung im MVP.* Das Terminal-Widget (`xterm2`) hat OSC 52, OSC 8 und das Setzen des
Fenstertitels abgeschaltet. Über dem Terminal steht dauerhaft der Hinweis, dass die Ausgabe des
Agenten nicht vertrauenswürdig ist.

*Restrisiko.* Fehler im Widget; Kopieren per Maus bleibt möglich und ist gewollt.

*Status.* MVP.

*Prüfung.* ESC-5 sendet eine OSC-52-Sequenz und prüft, dass die Host-Zwischenablage unverändert
bleibt.

### K-10 DNS und Auflösungs-Zeitpunkt

*Was er ist.* Ein Hostname trägt bis zu 63 Byte pro Label. Wer `<63 Byte kodierte Daten>.attacker.com`
anfragt, hat exfiltriert, sobald irgendjemand diesen Namen auflöst — der autoritative Server des
Angreifers sieht die Anfrage, auch wenn die HTTP-Verbindung nie zustande kommt. Ein Proxy, der
vor der Entscheidung auflöst, ist damit selbst der Exfiltrationskanal.

*Schwere.* Hoch.

*Minderung im MVP.* Aufgelöst wird ausschließlich **nach** einer Freigabe, genau einmal, über den
`Resolver`-Port, nie über den System-Resolver eines HTTP-Connectors. Die gefundene IP wird an die
Verbindung gepinnt (`Egress::connect(authority, Some(ip))`), damit ein zweiter Lookup keine andere
Adresse liefern kann (DNS-Rebinding). Löst der Name auf eine private, Loopback-, Link-Local- oder
CGNAT-Adresse auf, wird die Verbindung verweigert, außer die Regel trägt `allow_private: true`.

*Restrisiko.* Nach der Freigabe fließt der Name naturgemäß ab. Außerdem bleibt der *Zeitpunkt*
einer Freigabe für einen Beobachter im Netz sichtbar; ein Angreifer kann daraus ein langsames
Ein-Bit-Signal bauen, wenn der Mensch bereitwillig freigibt.

*Status.* MVP (ADR-006, umgesetzt in HUM-024).

*Prüfung.* ESC-3 beobachtet host-seitig, dass vor der Entscheidung kein Lookup für den
angefragten Namen stattfindet.

### K-11 Queue- und Regel-Metadaten

*Was er ist.* Der Agent erfährt aus dem Verhalten des Systems, was erlaubt ist: eine sofortige
Antwort bedeutet „Regel greift", ein `403` mit `reason: rule` bedeutet „ausdrücklich verboten",
eine lange Verzögerung bedeutet „ein Mensch schaut hin". Zusätzlich beantwortet der Proxy den
virtuellen Host `humanitl.internal` mit Status und geltenden Regeln.

*Schwere.* Niedrig. Es ist ein Orakel über die Policy, kein Datenabfluss.

*Minderung im MVP.* `humanitl.internal` ist absichtlich lesend gehalten und wird ohne Upstream
beantwortet; `/ask` erzeugt eine Karte im UI und niemals eine Regel. Der Agent bekommt damit nur
das, was ihm ohnehin durch Ausprobieren zugänglich wäre, aber ohne die Queue mit Rateversuchen zu
fluten. Blockmeldungen enthalten optional eine Notiz des Menschen, was Rateversuche unnötig macht.

*Restrisiko.* Das Orakel bleibt bestehen und ist gewollt: ein Agent, der weiß, dass PyPI erlaubt
und GitHub gesperrt ist, belästigt den Menschen seltener.

*Status.* MVP.

*Prüfung.* ESC-4 prüft die Regel-Tabelle; `curl http://humanitl.internal/` in der Sandbox zeigt
die Antwort.

### K-12 Package-Caches

*Was er ist.* Ein zwischen Projekten geteilter Cache (`~/.npm`, `~/.cache/pip`, `~/.cargo`) trägt
Daten aus einem Projekt in ein anderes: ein präpariertes Paket, eine untergeschobene Datei, oder
schlicht Quelltext, den ein Post-Install-Skript dort ablegt.

*Schwere.* Mittel.

*Minderung im MVP.* Caches werden pro Projekt gehalten, nicht pro Vertrauensstufe geteilt. Wer
keinen Cache konfiguriert, bekommt keinen; dann kostet jede Installation Netz und damit eine
Freigabe.

*Restrisiko.* Wer einen gemeinsamen Cache einträgt, um Zeit zu sparen, baut den Kanal wieder auf.

*Status.* MVP (Trennung), später schreibgeschützte Seed-Caches.

*Prüfung.* `humanitl sandbox argv` zeigt jeden Cache-Mount namentlich.

### K-13 Direkter Netzwerk-Egress

*Was er ist.* Der Kanal, den man erwartet: eine TCP-Verbindung ins Internet, ein UDP-Paket, ein
QUIC-Handshake, ein ICMP-Tunnel, ein Raw-Socket.

*Schwere.* Kritisch, wenn er bestünde.

*Minderung im MVP.* Er besteht nicht. Es gibt kein Interface außer `lo`, keine Route, keine
Capability, um eines anzulegen, und der Filter verweigert `SOCK_DGRAM` und `SOCK_RAW` ebenso wie
jede Familie außer `AF_INET`/`AF_INET6`. Abstrakte Unix-Sockets sind an das Netz-Namespace
gebunden und damit ebenfalls tot.

*Restrisiko.* Ein Kernel-Fehler. Nichts, was Humanitl abfangen könnte.

*Status.* MVP.

*Prüfung.* ESC-1 (Sockets, Interfaces, Routing) und ESC-3 (Egress ohne Proxy).

### K-14 Audit-Datei auf dem Host

*Was er ist.* Die Entscheidungshistorie liegt als JSONL-Datei im Datenverzeichnis. Wer sie ändert,
ändert die Antwort auf die Frage „was ist damals rausgegangen".

*Schwere.* Hoch für das zweite Schutzziel.

*Minderung im MVP.* Jeder Eintrag trägt `seq`, `ts`, `prev_hash` und `hash` über kanonischem JSON;
darüber liegt ein HMAC mit einem Installationsschlüssel aus dem Keyring. `humanitl audit verify`
prüft die Kette.

*Restrisiko.* Ausführlich in [`SECURITY.md`](SECURITY.md) Abschnitt 8. Kurz: Erkannt wird das
nachträgliche Ändern durch jemanden, der Dateizugriff hat. Nicht erkannt wird ein Angreifer, der
als derselbe Nutzer läuft und damit Schlüssel **und** Datei besitzt — er baut die Kette neu.
Ebenso wenig erkannt wird das Kürzen des Endes hinter dem letzten Anker.

*Status.* MVP (Kette), externes Anchoring später.

*Prüfung.* `humanitl audit verify`; ESC-5 löscht einen Eintrag und kürzt die Datei und erwartet in
beiden Fällen einen Befund.

---

### K-15 Das Fenster zwischen `exec` und `SIGKILL`

*Was er ist.* Der Shim meldet die drei Garantien und startet den Agenten unmittelbar danach; er
erzwingt nichts (`CONVENTIONS.md` 4.12). Der Wirt liest den Bericht erst danach. Zwischen dem
`exec` des Agenten und dem `SIGKILL` des Daemons läuft der Agent in einer Sandbox, deren Isolation
noch nicht belegt ist: Die Brücke steht, der Proxy nimmt an, eine `allow`-Regel leitet ohne
Menschen weiter, und die Durchreiche zum Modell streamt.

*Schwere.* Hoch. Im Fall `SANDBOX_015` ist genau der zweite Socket, den die Prüfung beanstandet, in
diesem Fenster benutzbar.

*Minderung im MVP.* Das Fenster ist so kurz wie es ohne den Proxy geht: Der Bericht wird sofort
nach dem Start gelesen, und der Daemon beendet die Sandbox ohne Gnadenfrist (`SIGKILL` statt
`SIGTERM` plus fünf Sekunden) und wartet nur einmal. Im Normalfall sind das Millisekunden.
Zusätzlich hält jede Anfrage, die keine Regel entscheidet, ohnehin an einem Menschen — das Fenster
wirkt nur über Regeln, die schon `allow` sagen, und über die erklärte Durchreiche.

*Obere Schranke.* Nicht eine Frist, sondern ihre Summe: `REPORT_TIMEOUT` (5 s), bis ein
ausbleibender Bericht als ausgeblieben gilt, plus `KILL_GRACE` (5 s) für das Einsammeln nach dem
`SIGKILL` — im Daemon zusammen **bis zu 10 s**. Auf den Wegen `humanitl sandbox run` und
`escape-launch` kommt eine weitere `KILL_GRACE` dazu, weil sie mit `SIGTERM` beginnen: **bis zu
15 s**.

*Restrisiko.* Ein Agent, der in dieser Zeit gezielt eine erlaubte Verbindung aufbaut.

*Status.* Offen, und auf **jedem** Weg in die Sandbox gleich: `escape-launch` und `humanitl sandbox
run` starten ebenso erst und prüfen danach. Keiner von ihnen ist fail-closed in dem Sinn, dass der
Befehl nicht liefe. Es zu schließen heißt, dass der Proxy Verbindungen ablehnt, solange die
Isolation der laufenden Sitzung nicht belegt ist; das ist ein eigenes Issue (`humanitl-proxy` liegt
innerhalb von `humanitl-ipc`, die Naht gehört deshalb in eine innere Crate). Bis dahin gilt: Ein
roter Isolations-Check **beendet** die Sitzung, er **verhindert** sie nicht.

*Prüfung.* `daemon/crates/ipc/tests/sandbox_start.rs`
(`a_second_socket_in_the_project_stops_the_start`) belegt, dass die Sitzung beendet wird und nie
`running` meldet. Dass in dem Fenster nichts hinausgeht, ist **nicht** belegt. Die Escape-Tests
belegen es auch nicht: Sie laufen selbst in dieser Sandbox und messen von innen, was dort gilt —
über die Zeit zwischen `exec` und `SIGKILL` sagen sie nichts.

## 5. Annahmen

Die Argumentation dieses Dokuments steht und fällt mit sechs Annahmen. Die ersten vier sind die
tragenden, die letzten beiden die selbstverständlichen, die trotzdem jemand aufschreiben muss.

| ID | Annahme | Wie man sie prüft |
|---|---|---|
| A-1 | `bubblewrap` ≥ 0.8 ist installiert und darf nicht-privilegierte User-Namespaces anlegen | `bwrap --version`; `sysctl kernel.unprivileged_userns_clone` beziehungsweise `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` |
| A-2 | Der Kernel ist ein 5.x oder neuer (getestete Untergrenze 5.10) und hat seccomp-BPF mit `SECCOMP_FILTER_FLAG_TSYNC` | `uname -r`; `grep CONFIG_SECCOMP_FILTER /boot/config-$(uname -r)`; `grep Seccomp /proc/self/status` |
| A-3 | Der Mensch liest, was er freigibt | Nicht technisch prüfbar. Das UI ist darauf ausgelegt, das Lesen billig zu machen: Findings zuerst, Body vollständig, Ziel als Klartext-Identität |
| A-4 | Der LLM-Host ist unter Kontrolle des Nutzers | `humanitl config get llm.endpoint`; Zeile 4 des Isolations-Panels |
| A-5 | Der Host ist nicht kompromittiert (kein Root-Angreifer, kein manipuliertes Humanitl-Binary) | Paketsignatur, `systemctl --user status humanitld` |
| A-6 | Die Sandbox-Profile sind die ausgelieferten oder vom Nutzer bewusst geänderten | `humanitl sandbox argv` zeigt die vollständige Kommandozeile, `git diff` auf `profiles/` |

---

## 6. Wenn eine Annahme bricht

**A-1 bricht** (bwrap fehlt, ist zu alt oder User-Namespaces sind gesperrt). Dann startet keine
Sitzung. Humanitl fällt nicht auf eine schwächere Isolation zurück und bietet auch kein „trotzdem
starten" an; es liefert ein `Diagnostic` mit dem konkreten Befehl, der das Problem behebt
(Paket installieren, `apparmor_restrict_unprivileged_userns` setzen). Der Ausfall ist laut und
vollständig, weil ein stiller Rückfall genau die Behauptung zerstören würde, für die es das
Programm gibt.

**A-2 bricht** (Kernel zu alt oder seccomp deaktiviert). Das Shim-Kind kann den Filter nicht
setzen und bricht vor dem `exec` des Agenten ab, Exit-Code 3, Diagnostic `SANDBOX_016` (`SANDBOX_013`, falls kein Prüfbericht übermittelt wurde). Die
erste und zweite Garantie stünden weiterhin, doch die dritte fehlt, und die dritte ist die, die den Fall abdeckt,
in dem eine Abhängigkeit eigenen Code ausführt. Darum ist das ein Startabbruch und keine Warnung.

**A-3 bricht** (der Mensch klickt durch). Dann ist Humanitl ein Aufzeichnungsgerät und kein
Schutz. Technisch bleibt nur die Historie: Es ist danach nachvollziehbar, was freigegeben wurde.
Gegenmaßnahmen sind Ergonomie, nicht Kryptographie: Gruppierung nach Host, Findings vor dem Body,
Regeln, die den Lärm wegnehmen, damit die verbleibenden Fragen Aufmerksamkeit bekommen, und eine
Rückfrage beim pauschalen Freigeben ganzer Gruppen. Der Timeout endet in `block`, damit
Unaufmerksamkeit auf der sicheren Seite landet.

**A-4 bricht** (der LLM-Host ist fremd, geteilt oder kompromittiert). Dann ist der Modell-Kanal
ein Vollzugriff auf alles, was der Agent an das Modell schickt, und keine Sandbox der Welt hilft
dagegen: Der Kanal ist gewollt offen. Der Setup-Text sagt deshalb „nur eine Maschine, die du
kontrollierst". Wer kein solches Gerät hat, sollte den Passthrough abschalten und auch die
Inferenz halten lassen — das ist unbequem und ehrlich.

**A-5 bricht** (Root-Angreifer oder manipuliertes Binary). Alle Aussagen dieses Dokuments sind
hinfällig. CA-Schlüssel, Audit-Schlüssel und der Speicher des Daemons liegen offen. Es gibt keine
Teil-Verteidigung, die hier noch etwas trägt.

**A-6 bricht** (ein Profil enthält einen zusätzlichen Mount oder eine zusätzliche Socket-Familie).
Der Isolations-Check schlägt beim Start an, wenn ein unerwarteter Socket in der Sandbox auftaucht,
und verhindert den Start. Mounts, die nur Dateien öffnen (etwa `~/.ssh` lesend), erkennt er nicht;
dagegen hilft nur, dass die vollständige `bwrap`-Zeile im UI sichtbar ist und in der
Sitzungs-Historie steht.

---

## 7. Änderungshistorie

| Datum | Änderung | Anlass |
|---|---|---|
| 2026-09-02 | Erstfassung: vier Angreifer, vierzehn Kanäle, sechs Annahmen | HUM-007, Sicherheits-Review vom selben Tag |
| 2026-09-02 | Review-Korrekturen: Shim-Prozessmodell (Brücke im Elternprozess, Filter im Kind), CA-Schlüssel bleibt auf dem Host, kein Loopback-Port auf dem Host, Mount-Allowlist als Auszug der Argv-Tabelle, `socketpair()` bleibt unberührt und erlaubt (CONVENTIONS.md 4.11) | HUM-007 Review |
| 2026-09-04 | K-15 aufgenommen: der Isolations-Check läuft, nachdem der Shim den Agenten gestartet hat, also beendet ein roter Check die Sitzung, statt sie zu verhindern. Der Halbsatz „Check 3 prüft, dass `socketpair` gelingt" gestrichen — `probe_families` probt es nicht | HUM-041, externer Review |

Geplante Fortschreibung: HUM-059 bringt das Dokument zum Release auf den Stand des Codes. Jede
sicherheitsrelevante Änderung am Shim, an der Mount-Allowlist, am Filter oder am Passthrough
bekommt eine Zeile in dieser Tabelle.

---

## Schwachstellen melden

Bitte keine öffentlichen Issues für Sicherheitsprobleme. Der Meldeweg und die Frist stehen in
[`SECURITY.md`](SECURITY.md) im Abschnitt „Schwachstellen melden".
