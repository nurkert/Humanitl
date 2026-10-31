---
source: SECURITY.md
source-sha256: ef83c66616b66b452e804fa3b482e9a604a17ddf9b3ed5ed77c8696302e992b2
lang: de
---
# Sicherheit

Applies to: Humanitl 0.1

Dieses Dokument erklärt, warum ein Agent unter Humanitl das Netzwerk nicht unbemerkt erreichen
kann, wo diese Aussage endet und wie du beides selbst prüfst. Es richtet sich an zwei Leser: an
Fachleute ohne Sicherheitshintergrund, die wissen wollen, worauf sie sich verlassen können, und
an Prüfer, die jede Behauptung an einem Befehl oder einer Datei nachprüfen. Wer was angreift,
beschreibt [`THREAT-MODEL.md`](THREAT-MODEL.md); dieses Dokument beschreibt die Mechanismen.

Wo dieses Dokument und der Code sich widersprechen, hat der Code recht und dieses Dokument einen
Fehler. Die seccomp-Regeltabelle, das Sandbox-Profil und das Konfigurationsschema sind die
Quellen; die Stellen, die sie zitieren, werden durch Tests geprüft (`security_doc_mentions_syscalls` in
`daemon/bin/humanitl-shim/src/seccomp.rs`, die erzeugte [`CONFIG.md`](CONFIG.md)).

## 1. Die Aussage

> Die Sandbox hat kein Netzwerk-Interface, es gibt keinen Weg nach draußen, und der Kernel
> verweigert jede Socket-Familie außer Loopback-TCP. Ihre einzigen Wege hinein und hinaus sind
> das Projektverzeichnis `/work` und genau ein Unix-Socket zum Proxy. Jedes Byte auf diesem
> Socket ist entweder eine HTTP-Anfrage, die du freigegeben hast, oder ein Aufruf an den
> erklärten LLM-Host, und beides wird aufgezeichnet.

„Die du freigegeben hast" heißt: du, in dem Moment, in dem die Anfrage angehalten war, oder
vorab durch eine Regel in deinem Regelwerk (deine eigenen Regeln oder die mitgelieferten, die
du eingeschaltet gelassen hast). Die Freigabe bestimmt, was der Proxy zum Ziel weiterleitet.
Anfragen, auf die eine `allow`-Regel passt, und Aufrufe an den erklärten LLM-Host gehen ohne
Anhalten hinaus. „Beides wird aufgezeichnet" gilt, solange der Recorder arbeitet: Während eines
Ausfalls der Aufzeichnung (HUM-250) läuft der Verkehr unaufgezeichnet weiter, und jeder Client
erfährt das ([Abschnitt 5.6](#56-other-known-limits), Punkt 12). Die Aussage ruht auf
drei Garantien. Sie stehen wörtlich in der Isolationsanzeige der Anwendung (ARB-Schlüssel
`isolationCheck1` bis `isolationCheck3`) und in `BACKLOG.md` 4.1:

| # | Garantie | Geprüft durch |
|---|---|---|
| 1 | **Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.** | [Abschnitt 2](#2-guarantee-1-no-network-interface), ESC-1, ESC-3 |
| 2 | **Genau eine Tür: ein Socket, der zu Humanitl führt.** | [Abschnitt 3](#3-guarantee-2-exactly-one-door), ESC-2 |
| 3 | **Der Kernel öffnet keine neue Tür (seccomp).** | [Abschnitt 4](#4-guarantee-3-no-new-doors), ESC-1 |

Unter macOS ist die Sandbox Seatbelt statt bwrap, und die drei Garantien lauten dort anders;
[Abschnitt 12](#12-macos-seatbelt-instead-of-bubblewrap) nennt sie und was sich ändert.

### Was die Aussage nicht sagt

Sie sagt **nicht** „der Agent kann keine Daten ausleiten". Zwei Kanäle sind bewusst offen,
in der Anwendung angezeigt und in [Abschnitt 5](#5-declared-side-channels) beschrieben:

1. **Das Projektverzeichnis.** Der Agent schreibt nach `/work`. Was er dort hinterlässt, liegt
   danach auf deiner Platte, auch etwas, das später ein Mensch selbst hinausträgt.
2. **Die Verbindung zum Sprachmodell.** Der Agent schickt Code an das Modell, ohne dass jede
   Inferenz bestätigt wird. Wer den Modell-Host kontrolliert, sieht diesen Verkehr.

Die genaue Aussage lautet: Der Agent kann **über das Netzwerk** nichts ausleiten, außer durch
Anfragen, die freigegeben wurden, von einem Menschen oder durch eine Regel, und über den
erklärten LLM-Host. Der Satz „der LLM-Host gehört zur vertrauenswürdigen Basis" ist die
Bedingung, unter der alles andere gilt.

Garantie 3 heißt **nicht** „gar keine Sockets". Der Filter muss TCP über IPv4 und IPv6
erlauben, denn so erreicht der Agent den Proxy auf `127.0.0.1:3128`. Dass ein erlaubter
TCP-Socket trotzdem nirgendwohin führt, leistet der leere Netzwerk-Namespace aus Garantie 1,
nicht der Filter: Dort gibt es nur `lo`, und die Routing-Tabelle ist leer. Die drei Garantien
gelten zusammen; keine gilt allein.

### Die eigenen Erlaubnisabfragen des Agenten (HUM-308)

Agenten bringen eigene Erlaubnisabfragen mit: OpenCode fragt „allow webfetch?", bevor es eine Seite
abruft, und kann vor einem Shell-Befehl oder einer Bearbeitung fragen. In der Sandbox schaltet
Humanitl diese für das Abrufen und das Ausführen von Befehlen ab (`"webfetch": "allow"`,
`"bash": "allow"` in der mitgelieferten `opencode.json`, ebenso in `OPENCODE_PERMISSION`);
`edit` bleibt auf der eigenen Vorgabe von OpenCode, die es ohnehin ohne Erlaubnisabfrage erlaubt.
pi 1.4.0 hat keine solchen Erlaubnisabfragen. Keine der Garantien oben ruht auf diesen Erlaubnisabfragen,
und sie abzuschalten schwächt keine davon:

- **Sie sind keine Grenze.** Die Erlaubnisabfrage läuft im Prozess des Agenten, der auf der nicht
  vertrauenswürdigen Seite von B1 sitzt ([`THREAT-MODEL.md`](THREAT-MODEL.md#trust-boundaries)).
  Ein gesteuerter oder kompromittierter Agent fragt nicht; eine Anfrage, die er trotzdem
  schickt, wird am Proxy genauso angehalten.
- **Die Netzwerkfrage wird einmal gestellt, an der Tür.** Jede Anfrage geht nur durch den
  Proxy hinaus und wird dort freigegeben, durch eine Regel oder durch dich. Eine zweite
  Erlaubnisabfrage vor derselben Anfrage würde dich zweimal nach einer Entscheidung fragen und dir
  beibringen, durchzuklicken.
- **Shell und Bearbeitungen bleiben in der Sandbox.** Befehle laufen unter denselben Namespaces
  und demselben Filter; Schreibzugriffe, die die Session überdauern, erreichen nur `/work`
  ([Abschnitt 5.2](#52-the-project-directory-work)), das du danach in der Zusammenfassung des
  Laufs prüfst, dazu jedes `mounts.extra_rw`, das ein Profil erklärt, und eingerichtete
  Paket-Caches ([Abschnitt 5.5](#55-package-caches)). `/tmp` und das Home des Agenten sind ein
  `tmpfs` und enden mit der Session. `external_directory` bleibt `deny`, und `websearch`
  bleibt `deny`.

Was beim Agenten bleibt, ist die Frage, die er zu seiner eigenen Arbeit noch stellen darf, etwa
die Erlaubnisabfrage von OpenCode, bevor es eine `.env`-Datei liest oder nachdem derselbe Aufruf
dreimal wiederholt wurde. Keine davon ist eine Netzwerkentscheidung, und beide bleiben auf der
Vorgabe von OpenCode.

### Erweiterungen von Drittanbietern für Agenten (HUM-309)

Mit `agent.pi.web_access` auf `auto` (der Vorgabe) oder `on` lädt pi das npm-Paket
`pi-web-access`, wenn du es global installiert hast. Das ist Code von Drittanbietern, der im
Prozess des Agenten läuft. Er sitzt also wie der Agent selbst auf der nicht vertrauenswürdigen
Seite von B1 und bekommt kein Recht, das der Agent nicht hat: dieselben Namespaces, derselbe
seccomp-Filter und dieselben Mounts; sein Verzeichnis liegt in einem schreibgeschützten Mount
(normalerweise `/usr`); er geht nur durch den Proxy hinaus, und über jede Anfrage, die er
stellt, entscheidet eine Regel, oder sie wird, wenn keine Regel entscheidet, für dich angehalten wie jede andere (gemessen: ein Abruf kam am Proxy als `CONNECT` an). Sein eigener
SSRF-Schutz ist angewiesen, die Namensauflösung dem Proxy zu überlassen (`ssrf.trustEnvProxy`),
weil die Sandbox keinen Resolver hat; das ändert keine Garantie, denn der Proxy ist ohnehin die
einzige Tür. Kein API-Schlüssel eines Suchanbieters erreicht die Sandbox. Was die Aussage nicht
abdeckt, ist das, was du installierst: Humanitl prüft das Paket nicht, eine bösartige Version
kann also in der Sandbox tun, was ein gesteuerter Agent dort tun könnte.
`agent.pi.web_access = off` hält es draußen, und ein Projekt-Profil kann das nicht umschalten.

### Vertrauenswürdige Basis

Die vertrauenswürdige Basis (Trusted Computing Base, TCB) ist die Menge der Bauteile, deren
Versagen die Garantien aufhebt. Sie ist bewusst klein und besteht aus Bauteilen, die viele
andere Projekte ebenfalls nutzen.

| Bauteil | Rolle | Anforderung | Warum wir ihm vertrauen |
|---|---|---|---|
| Linux-Kernel | Namespaces, seccomp-BPF, Capabilities | 5.4 oder neuer (das Minimum, das `humanitl doctor` prüft), getestet ab 5.10; `CONFIG_SECCOMP_FILTER`, unprivilegierte User-Namespaces erlaubt | dieselben Mechanismen tragen Container, Flatpak und die Sandboxen von Claude Code und der OpenAI Codex CLI |
| `bubblewrap` | baut die Sandbox | 0.8 oder neuer (`min_bwrap_version` in `profiles/sandbox/default.toml`); 0.10 oder neuer für das Binden über Deskriptoren | klein, im rootless-Modus ohne setuid, von Debian und Fedora paketiert, seit Jahren unter Beobachtung |
| seccomp-BPF-Filter | verweigert weitere Socket-Familien und gefährliche Syscalls | erzeugt von `seccompiler` plus einem handgeschriebenen Vorspann, Quelle `daemon/bin/humanitl-shim/src/seccomp.rs` | ein paar Dutzend Regeln, von einem Test vollständig aufgezählt |
| `humanitl-shim` | hält die Bridge, forkt, installiert den Filter im Kindprozess, `exec` | Rust, kein tokio, nur `libc` und `seccompiler` | das Prozessmodell ist sein ganzer Inhalt; sechs Dateien |
| `humanitld` | Proxy, Warteschlange der angehaltenen Anfragen, Regeln, Aufzeichnung, Steuerung der Sandbox | Rust; `unsafe` ist in den Bibliotheks-Crates verboten außer an einem Pre-exec-Hook in `humanitl-sandbox/src/inherit.rs`, und im Binary nur in `bin/humanitld/src/systemd.rs` (Entfernen der `LISTEN_*`-Variablen) | speichersicher, kein Interpreter, kein Paketbaum zur Laufzeit |
| `hyper` 1, `tokio-rustls`/`rustls`, `rcgen`, mit eigener Accept-Schleife (`daemon/crates/proxy/src/core.rs`) | TLS-Terminierung und HTTP-Verarbeitung | Versionen in `daemon/Cargo.lock` festgelegt, regelmäßig aktualisiert | ein TLS-Stack in Rust ohne die OpenSSL-Klasse von Speicherfehlern; die Geschichte von Rapid Reset und CONTINUATION-Flood wird verfolgt |
| Zertifikatsspeicher des Systems | Vertrauensanker für die eigenen Verbindungen des Daemons zu Zielen (HUM-257) | `/etc/ssl/certs`, nur von root beschrieben; `webpki-roots`, wenn er keine brauchbare Wurzel enthält | der Speicher, dem `curl`, `apt` und der Browser vertrauen; wer ihn ändert, ist root und außerhalb des Modells |
| der LLM-Host | sieht jeden Prompt | muss dem Nutzer gehören | ein erklärter Seitenkanal, keine technische Absicherung ([Abschnitt 5.1](#51-the-llm-passthrough)) |
| Audit-Schlüsseldatei | HMAC-Schlüssel der Audit-Kette | `$XDG_DATA_HOME/humanitl/keys/audit.key`, `0600` in `0700` | schützt gegen andere Konten, nicht gegen dasselbe Konto |
| Schlüsselbund des Systems (Secret Service, unter macOS der Anmelde-Schlüsselbund) | der Zugangsschlüssel für das LLM (HUM-140, HUM-349) | `secret-tool` aus `libsecret-tools`, unter macOS `/usr/bin/security`; ohne Antwort `$XDG_DATA_HOME/humanitl/keys/llm.key`, `0600` in `0700` | schützt gegen andere Konten, nicht gegen dasselbe Konto; der Agent liest den Schlüssel in der Sandbox, weil er ihn braucht |

**Nicht Teil der Isolations-TCB: die Flutter-Anwendung.** Sie ist ein gRPC-Client wie die
Kommandozeile und kann die Sandbox nicht schwächen. Für die *Entscheidung* wird ihr aber
vertraut: Sie zeigt dem Menschen, worüber er entscheidet. Eine Anwendung, die einen Body falsch
darstellt, führt zu einer falschen Freigabe; deshalb werden Bodies als Text dargestellt und nie
in einer Web-Ansicht.

**Ebenfalls nicht Teil der TCB: das Projekt-Profil** `<project>/.humanitl/profile.toml`. Es
liegt im geklonten Repository. Ein Schlüssel, der einen Schutz des Nutzers aufheben würde, wird
dort mit `CONFIG_003` verweigert, und der Daemon startet nicht. Das betrifft
`hold.hard_block_checksum_secrets` (HUM-208) und `limits.recorder_max_body_bytes` (HUM-209):
Vorher konnte ein Projekt-Profil die harte Sperre für IBANs und Kartennummern abschalten oder
die Aufzeichnung jedes Bodys auf 1 KiB verkleinern, während eine Freigabe den ganzen Body
schickte. Die vollständige Liste ist die Spalte „Projekt" in
[`CONFIG.md`](CONFIG.md).

Toolchain für reproduzierbare Builds: Rust 1.95.0 (festgelegt in `daemon/rust-toolchain.toml`,
mindestens 1.88), Flutter 3.47.2 (festgelegt in `app/.fvmrc`), `bubblewrap` 0.8 oder neuer.
`socat` wird **nicht** gebraucht: Die Bridge lebt im Shim, damit kein weiteres Programm in der
Sandbox läuft.

## 2. Garantie 1: Kein Netzwerk-Interface

**Mechanismus.** Die Sandbox wird mit `bwrap` gestartet, mit den im Profil ausgeschriebenen
Namespaces von `--unshare-all` (`unshare = ["user", "pid", "net", "ipc", "uts", "cgroup"]`; ein
Profil, das einen davon weglässt, wird verweigert). Bubblewrap ist ein kleines, weit verbreitetes
Programm, das ohne root neue *Namespaces* anlegt: getrennte Sichten des Kernels auf Netzwerk,
Prozesse, Mounts, IPC und Hostname. Im neuen Netzwerk-Namespace gibt es genau ein Gerät, das
Loopback-Interface `lo`, das bwrap hochfährt, damit die Bridge zum Proxy auf `127.0.0.1` lauschen
kann. Es gibt keine Adresse nach außen, keine Route, keinen DNS-Server, kein ICMP, kein QUIC,
keinen Raw-Socket. bwrap läuft mit `--cap-drop ALL` und `--disable-userns` (`bwrap_args.rs`):
Der Agent hält keine Capabilities und kann keinen weiteren User-Namespace anlegen, in dem er sie
zurückbekäme. Also kann niemand ein Gerät hinzufügen: `CAP_NET_ADMIN` fehlt.

Was das im Kernel bedeutet: Ein Netzwerk-Namespace ist ein vollständiger, getrennter
Netzwerk-Stack. Ein darin angelegter Socket kann nur Interfaces dieses Namespaces erreichen. Mit
nur `lo` und einer leeren Routing-Tabelle endet `connect()` zu jeder Adresse, die nicht Loopback
ist, mit `ENETUNREACH`. Ein abstrakter Unix-Socket braucht keinen Pfad im Dateisystem, aber
abstrakte Sockets sind an den Netzwerk-Namespace gebunden, und dieser Namespace ist leer.

**Prüfung (in der Sandbox).**

```sh
ip link                 # only "lo"; without iproute2: cat /proc/net/dev
ip route                # empty
cat /proc/net/route     # only the header line
capsh --print           # Current: = (empty)
```

**Automatisiert.** `tests/escape/esc-1-sockets.sh` (ESC-1) und `tests/escape/esc-3-egress.sh`
(ESC-3). Zur Laufzeit meldet der Shim dasselbe als `IsolationCheck::NoNetworkInterface`
(`CHECK no_interfaces`); schlägt die Prüfung fehl, hält der Rechner die Sandbox an, meldet
`SANDBOX_014` und behandelt sie nie als laufend. Das heißt **nicht** „der Agent startet nicht":
siehe [das Fenster zwischen `exec` und `SIGKILL`](#the-window-between-exec-and-sigkill).

**Was ein Angreifer versuchen würde.** Ein `veth`-Paar oder ein TUN-Gerät anlegen: scheitert
ohne `CAP_NET_ADMIN`. Ein UDP-Paket an einen DNS-Server schicken: scheitert schon am Filter,
spätestens an der fehlenden Route. Einen vom Rechner geerbten Deskriptor benutzen: Der Daemon gibt
nur die Deskriptoren in `LaunchPlan::fds` weiter, und der Shim schließt zusätzlich alles, was
ein Starter ohne `CLOEXEC` vererbt hat, bevor er forkt.

## 3. Garantie 2: Genau eine Tür

**Mechanismus.** Der Daemon legt seinen Proxy-Socket auf dem Rechner unter
`$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` an, in einem eigenen Verzeichnis mit Modus `0700`.
Die **Datei** wird unter `/run/humanitl/proxy.sock` in die Sandbox gebunden, nie das Verzeichnis.
Der Unterschied zählt: Ein gebundenes Verzeichnis würde jede später dort angelegte Datei zeigen,
und der Agent könnte selbst Sockets darin anlegen. Der gRPC-Socket des Daemons
(`$XDG_RUNTIME_DIR/humanitl/daemon.sock`, Modus `0600`) existiert in der Sandbox nicht; nur die
Anwendung und die Kommandozeile auf dem Rechner steuern den Daemon darüber.

In der Sandbox lauscht der Shim auf `127.0.0.1:3128` und leitet jede angenommene TCP-Verbindung
an diese eine Socket-Datei weiter. Die Umgebung des Agenten lässt `HTTP_PROXY`, `HTTPS_PROXY`
und `ALL_PROXY` dorthin zeigen und setzt `NO_PROXY` leer; `NODE_USE_ENV_PROXY=1` lässt das
eingebaute `fetch` von Node und die globalen Agents von `node:http` und `node:https` denselben Variablen folgen. Das Profil erlaubt genau eine Bridge: Eine zweite
wäre ein zweiter Listener zu einem zweiten Unix-Socket und wird mit `CONFIG_003` verweigert,
eine Bridge in Richtung `out` mit `SANDBOX_007`.

Alles andere, was der Agent braucht, kommt aus einer Erlaubnisliste, deren vollständige Form die
Argumentliste von bwrap ist (`humanitl sandbox argv`). Kurz gefasst, aus
`profiles/sandbox/default.toml`:

- schreibgeschützte Binds von `/usr`, `/etc/ssl`, `/etc/alternatives`, `/etc/ld.so.cache`,
  `/etc/localtime` und die Symlinks `/lib`, `/lib64`, `/bin`, `/sbin` nach `/usr`;
- das CA-Zertifikat von Humanitl als `/etc/humanitl/ca.crt`, erzeugte minimale `/etc/passwd`,
  `/etc/group` und `/etc/hosts` (aus dem Speicher des Daemons, nicht vom Rechner) und der Shim unter
  `/run/humanitl/humanitl-shim`;
- frisch angelegt: ein eigenes `/proc`, ein minimales `/dev`, `tmpfs` auf `/tmp`, `/var/tmp`,
  `/dev/shm` und ein leeres `/home/agent`;
- das Projekt unter `/work`, mit den Masken aus [Abschnitt 5.2](#52-the-project-directory-work);
- die Proxy-Socket-Datei;
- das Programm des Agenten, wenn es außerhalb dieser Binds liegt (HUM-278): eine native
  ausführbare Datei, schreibgeschützt unter `/run/humanitl/agent/<name>` gebunden, siehe unten.

Es gibt keine `/etc/resolv.conf`. Was **nicht** gebunden wird, steht im Code als Sperrliste,
die der Argument-Builder durchsetzt: `$XDG_RUNTIME_DIR`, `/run`, das `/tmp` des Rechners, `/home`,
`~/.ssh`, `~/.gitconfig`, `~/.netrc`, X11-, Wayland-, D-Bus- und Docker-Sockets. Ein Profil, das
eines davon nennt, wird mit `SANDBOX_006` verweigert. Der Shim und der Proxy-Socket liegen unter
`/run/humanitl`, dem einen Verzeichnis, das nie vom Rechner kommt.

**Das Programm des Agenten (HUM-278).** Agenten werden pro Nutzer installiert
(`~/.opencode/bin`, `~/.local/bin`, `~/bin`), und das sieht die Sandbox nicht. Ist der Befehl
in der Sandbox nicht startbar, durchsucht der Daemon seinen `PATH` und diese Verzeichnisse und
bindet die gefundene Datei schreibgeschützt unter `/run/humanitl/agent/<name>`; der Befehl nennt
dann diesen Pfad. Nur der eingerichtete Agent (`agent.command` oder die Vorgabe des Adapters)
wird so gebunden, nie ein Befehl, der nach `--` angegeben wird. Dieser Bind wird nach eigenen
Regeln geprüft, nicht nach der Mount-Sperrliste von `SANDBOX_006`, die alles unter `/home`
verweigern würde: Gebunden wird eine reguläre Datei, nie ihr Verzeichnis, und nur, wenn sie eine
native ausführbare ELF-Datei vom Typ `EXEC` oder `DYN` ist (ein Skript bräuchte Dateien daneben),
dem Nutzer oder root gehört und weder sie noch ihr Verzeichnis für jemand anderen beschreibbar
ist. Schreibrecht für die Gruppe zählt nur bei der privaten Gruppe des Nutzers: benannt wie der
Nutzer, ohne eingetragene Mitglieder und ohne ein anderes Konto, das sie als primäre Gruppe hat;
wo sich das nicht zeigen lässt, wird Schreibrecht für die Gruppe verweigert. Wie die Quellen von
`mounts.extra_ro` (HUM-263) wird die Datei einmal mit `O_PATH | O_NOFOLLOW` geöffnet, über
diesen Deskriptor geprüft (Eigentümer, Modus, ELF-Kopf) und über den Deskriptor gebunden
(`--ro-bind-fd`, oder `/proc/self/fd/N` bei bubblewrap älter als 0.10). Ein Austausch des Pfads nach der
Prüfung erreicht die Sandbox also nicht. Der Agent ist ohnehin nicht vertrauenswürdig; er
bekommt Lesezugriff auf den Code, den er schon ausführt, mehr nicht. Ein absolutes
`agent.command` folgt denselben Regeln.

**Eine Session mit dem Befehl des Aufrufers (HUM-280).** `humanitl run -- bash` bekommt dieselben
Adapter-Dateien und dieselbe Umgebung wie ein vom Daemon gestarteter Agent, und der
eingerichtete Agent wird nach genau den Regeln oben gebunden, neben dem Befehl des Aufrufers;
`/run/humanitl/agent` steht dann als erstes im `PATH` der Sandbox, sodass die Shell ihn über
seinen Namen findet. Der eigene Befehl des Aufrufers wird weiterhin nie gebunden: Er bleibt, wie
er geschrieben ist, und muss aus den Mounts des Profils startbar sein. Auf diesem Weg erreicht
nichts Neues die Sandbox. Der Bind ist dieselbe eine Datei, die ein vom Daemon gestarteter Agent
bekäme, die Schlüsseldatei ist die, die dieser Agent lesen würde, und das Verzeichnis im `PATH`
enthält diesen einen Bind und sonst nichts. `PATH` bekommt dieses Verzeichnis genau einmal und
als erstes, sodass ein gleichnamiges Programm in einem Verzeichnis wie `/work/bin` nicht vor dem
Bind stehen kann. Gibt der Schlüsselspeicher den Schlüssel nicht heraus (`LLM_009`), startet eine
Session mit dem Befehl des Aufrufers ohne ihn und meldet `LLM_009` als Warnung; dann wird keine
Schlüsseldatei geschrieben. Eine Session ohne Befehl verweigert den Start weiterhin.

**Gebunden wird, was geprüft wurde (HUM-263).** Das Projektverzeichnis und jeder Eintrag von
`mounts.extra_ro` und `mounts.extra_rw` werden beim Planen genau einmal geöffnet
(`O_PATH | O_NOFOLLOW`, das Projektverzeichnis zusätzlich mit `O_DIRECTORY`). Die Mount-Richtlinie
prüft dann den Ort, den `/proc/self/fd/N` für den geöffneten Deskriptor nennt, und bwrap bindet
genau diesen Deskriptor: ab bubblewrap 0.10 mit `--bind-fd` und `--ro-bind-fd`, darunter über
`/proc/self/fd/N` mit einem geerbten Deskriptor. Vorher prüfte der Starter den Pfad, und bwrap
löste ihn beim Einhängen ein zweites Mal auf; wer in ein Elternverzeichnis schreiben konnte,
konnte das Verzeichnis dazwischen gegen einen Symlink auf `~/.ssh` austauschen. Ein Austausch
nach dem Öffnen erreicht die Sandbox nicht mehr; ein Austausch zwischen Auflösen und Öffnen
scheitert am Vergleich und hält den Start mit `SANDBOX_006` an. Eine Profilquelle, die beim
Planen fehlt, wird auf dieselbe Weise verweigert.

Der Deskriptor selbst darf den Agenten nie erreichen: `openat(fd, "..")` von einem Deskriptor
eines Verzeichnisses des Rechners würde den Baum des Rechners an jedem Mount-Namespace vorbei hinaufgehen.
Mit `--bind-fd` schließt bwrap ihn. Im Ausweichweg über `/proc/self/fd/N` schließt ihn der Shim,
bevor er den Agenten startet, zusammen mit allem anderen, was er ohne `CLOEXEC` geerbt hat; der
Starter des Escape-Harness (`escape-launch`) verweigert deshalb den Start ohne brauchbaren Shim,
wenn bwrap älter als 0.10 ist. Im Daemon trägt der Deskriptor die ganze Zeit `FD_CLOEXEC`; nur
der bwrap-Kindprozess verliert es, zwischen `fork` und `exec`. Die Vorschau
(`humanitl sandbox argv`) hat keine Deskriptoren und zeigt weiter Pfade.

**Prüfung (in der Sandbox).**

```sh
find / \( -path /proc -o -path /sys -o \( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \) \) -prune -o -xtype s -print 2>/dev/null
                                                    # exactly one line: /run/humanitl/proxy.sock
cat /proc/self/mountinfo                            # no /run/user, no X11, no docker.sock
hostname                                            # "sandbox"
```

**Automatisiert.** `tests/escape/esc-2-mounts.sh` (ESC-2). Zur Laufzeit `IsolationCheck::SingleSocket`,
dessen Beleg aus der Sandbox selbst kommt: Vor `exec` durchläuft der Shim das Dateisystem der
Sandbox nach Unix-Sockets und meldet jeden, den er findet (`CHECK single_socket`), neben
`CHECK bridge_listening`, das zeigt, dass die eine Tür offen ist und antwortet. Der Durchlauf hat
ein Budget (ohne `/proc`, `/sys` und `/dev` außer `/dev/shm`, keinen Symlinks folgend, Tiefe 3,
2000 Einträge; wird eine Grenze erreicht, sagt der Beleg das). ESC-2 führt den unbegrenzten
`find`-Befehl oben aus (`esc_find_sockets` in `tests/escape/lib.sh`). `-xtype s`, nicht `-type s`: bwrap bindet den Proxy-Socket über eine leere reguläre Datei, und `find` vertraut bei einem bloßen `-type` dem Verzeichniseintrag; und kein `-xdev`, weil `/work`, `/tmp`, `/dev/shm` und `/home/agent` eigene Mounts sind, in denen ein eingeschleuster Socket auftauchen muss.

**Was ein Angreifer versuchen würde.** Den D-Bus- oder Docker-Socket suchen: nicht da. Über
`/proc/<pid>/root` einen anderen Mount-Namespace erreichen: Der eigene PID-Namespace zeigt nur
Prozesse der Sandbox. Neben dem Proxy-Socket einen zweiten Socket anlegen, um zwei Prozesse der
Sandbox an der Aufzeichnung vorbei zu verbinden: Das Verzeichnis ist nicht gebunden, und nach
dem Filter wird `AF_UNIX` ohnehin verweigert.

## 4. Garantie 3: Keine neuen Türen

**Mechanismus.** `humanitl-shim` ist ein kleines Programm ohne Laufzeitumgebung (nur `libc` und
`seccompiler`), das als PID 1 der Sandbox startet: Der Starter ruft bwrap mit `--as-pid-1` auf,
sodass kein Init von bwrap darüber sitzt (HUM-203). Sein Prozessmodell ist der eigentliche Kniff
(HUM-012): Die Bridge lebt im Elternprozess, der Agent ist ein Kindprozess, der den Filter vor
`exec` trägt. Beide tragen einen Filter; der des Elternprozesses erlaubt genau eine Familie mehr,
`AF_UNIX`, weil die Bridge sie braucht. Als PID 1 sammelt der Elternprozess jede Waise der
Sandbox ein (`waitpid(-1)`), zählt nur den Status seines eigenen Kindprozesses und leitet
`SIGTERM` und `SIGHUP` an den Agenten weiter; ohne Handler würde der Kernel sie bei einem
Namespace-Init verwerfen.

Die Reihenfolge, wie `daemon/bin/humanitl-shim/src/main.rs` sie ausführt:

1. **Bridge starten.** Für jede `in`-Bridge des Profils bindet der Elternprozess den TCP-Listener
   auf `127.0.0.1:3128` selbst, ohne `socat`, und verbindet sich einmal damit (`bridge_listening`).
   Der Listener existiert vor dem Fork, sodass der Agent nie `ECONNREFUSED` sieht, und trägt
   `CLOEXEC`, sodass der Agent ihn nicht erbt. Dann liest er die Interface-Liste
   (`no_interfaces`) und durchläuft das Dateisystem nach Sockets (`single_socket`).
2. **Schließen und forken.** Er schließt jeden geerbten Deskriptor außer 0, 1, 2, dem Bericht,
   dem Exec-Tor und den Listenern der Bridge, legt das `socketpair` an, über das der Listener des
   Filters reisen wird (HUM-138), und forkt. Der erste Schritt des Elternprozesses nach dem Fork
   ist `prctl(PR_SET_DUMPABLE, 0)`, mit `PR_GET_DUMPABLE` gegengeprüft, bevor das Tor aufgeht;
   danach installiert er seinen eigenen Filter, die Richtlinie des Agenten plus `AF_UNIX`.
3. **`PR_SET_NO_NEW_PRIVS` im Kindprozess.** Der Kindprozess stirbt mit dem Elternprozess
   (`PR_SET_PDEATHSIG`), schließt jeden geerbten Deskriptor außer 0, 1, 2, dem Bericht, dem Tor
   und seinem Ende des Kanals und setzt `PR_SET_NO_NEW_PRIVS`. Ab dann kann kein späteres `exec`
   Privilegien gewinnen, etwa über ein setuid-Programm; ohne das Flag darf ein unprivilegierter
   Prozess überhaupt keinen seccomp-Filter installieren.
4. **seccomp.** Der Kindprozess lädt den Filter mit einem Listener
   (`SECCOMP_FILTER_FLAG_NEW_LISTENER`): Ein verweigerter `socket()`-Aufruf wird an den
   Elternprozess übergeben, der ihn zählt und mit `EPERM` antwortet (siehe [was sichtbar wird](#what-becomes-visible-and-what-does-not)).
   Der Kindprozess reicht den Listener an den Elternprozess weiter und schließt seine eigene
   Kopie, bevor der Agent startet. Dieses Laden läuft ohne `TSYNC`, weil Kernel vor 5.7 `TSYNC`
   zusammen mit einem Listener verweigern und der Kindprozess direkt nach dem Fork genau einen
   Thread hat. Verweigert der Kernel den Listener (etwa mit `EBUSY`, weil Humanitl selbst in
   einer Sandbox läuft, die einen hat), lädt der Kindprozess denselben Filter ohne Listener und
   **mit** `SECCOMP_FILTER_FLAG_TSYNC`, und der Rechner meldet `SANDBOX_019`: genauso verweigert,
   nur zählt niemand mit. So oder so erbt jeder Thread und jeder Prozess, den der Agent später
   startet, den Filter.
5. **`exec`.** Der Kindprozess meldet `seccomp_applied` und `families`, wartet am Tor, bis der
   Elternprozess nicht mehr dumpbar und gefiltert ist, und ruft dann `execvp` auf den Agenten
   auf. Der Agent erbt den Filter und kann ihn nicht entfernen.

Der Elternprozess trägt seine eigene Fassung des Filters, weil ein Filter Threads abdeckt, keine
Elternprozesse: Der Agent kann seinen Filter nicht nach oben weitergeben, und der Elternprozess
kann den engeren Filter des Agenten nicht tragen, weil er `AF_UNIX` braucht. Beide Filter
verweigern dieselben Syscalls, also trägt jeder Prozess in der Sandbox einen Filter, PID 1
eingeschlossen (ESC-1 `seccomp_every_process`, `seccomp_parent_mode_2`). Bis HUM-203 war PID 1
das Init von bwrap: ungefiltert, dumpbar und unter der UID des Agenten. Mit
`kernel.yama.ptrace_scope = 0` konnte der Agent `/proc/1/mem` zum Schreiben öffnen und dort Code
ohne Filter ausführen (Sicherheitsdurchgang vom 2026-09-23, Befund M1).

### Weitere Shells in einer laufenden Sandbox (HUM-299)

`humanitl sandbox exec` öffnet eine Shell neben dem Agenten, wie `docker exec`. Niemand betritt
die Sandbox von außen: Es gibt kein `setns`, kein `nsenter`, und der Daemon startet selbst keinen
Prozess in den Namespaces der Sandbox. Er bittet PID 1, den Elternprozess des Shims, der die
Shell von innen forkt. Die Shell erbt deshalb, was jeder Prozess der Sandbox trägt:
`PR_SET_NO_NEW_PRIVS`, den Filter des Elternprozesses, dieselben Mounts, Namespaces, dieselbe
Umgebung (ohne die eigenen Variablen des Shims) und `/work` als Verzeichnis. Vor `exec`
installiert der Kindprozess der Shell zusätzlich den Filter des Agenten, in der Variante, die
selbst mit `EPERM` antwortet, und beweist ihn: `socket(AF_UNIX, SOCK_STREAM)`, das der Filter des
Elternprozesses für die Bridge noch erlaubt, muss mit `EPERM` antworten, sonst startet die Shell
nie. Filter stapeln sich nur, also trägt die Shell mindestens alles, was der Agent trägt. Sie
läuft auf einem eigenen Pseudo-Terminal, das der Daemon geöffnet hat, nimmt es wie der Agent als
steuerndes Terminal (`setsid`, `TIOCSCTTY` mit Argument 0), schließt jeden Deskriptor über
2 und setzt Signalmaske und Signalbehandlung zurück. Ihre Ausgabe läuft durch denselben
`FullScreen`-Filter wie die des Agenten ([Abschnitt 5.3](#53-the-agents-terminal-output)).

**Die Tür, und wer sie hält.** Die Bitte reist über ein `SOCK_SEQPACKET`-Paar, das der Starter
nur für einen Start am eigenen Pseudo-Terminal des Daemons anlegt. Der Daemon behält ein Ende
(`CLOEXEC`); bwrap erbt das andere und gibt es dem Shim als `--control-fd`. Das ist ein neuer
Weg in die Sandbox, also gehören seine Grenzen zur Aussage:

- Nur der Daemon kann bitten. Das Paar hat in keinem Dateisystem einen Namen, es gibt also
  nichts, womit man sich verbinden könnte, und `single_socket` findet weiterhin nur den
  Proxy-Socket. In der Sandbox hält nur PID 1 sein Ende: Der Kindprozess des Agenten schließt es
  vor seinem Filter, das Ende trägt `CLOEXEC`, jede weitere Shell schließt alle Deskriptoren über
  2, und PID 1 ist nicht dumpbar, sodass weder `/proc/1/fd` noch `pidfd_getfd(2)` (das die Grundsperre
  ohnehin verweigert) es erreicht. Auf der Seite des Daemons ist der einzige Aufrufer der
  `Exec`-RPC auf dem gRPC-Socket, der das Token des Daemons braucht und nie in die Sandbox
  gebunden wird.
- Eine Shell-ID ist `<session>-<n>`: die Zahl, die der Shim pro Sandbox ab 1 zählt, mit acht
  Hex-Zeichen der Sandbox-ID davor, sodass eine ID aus einer früheren Session nichts erreicht.
- Eine Bitte trägt einen Befehl und genau einen Deskriptor, die Follower-Seite des
  Pseudo-Terminals der Shell; alles andere wird verweigert. Zurück kommen Antworten von neun
  Byte (gestartet, verweigert, beendet) für IDs, nach denen der Daemon gefragt hat; der Daemon
  liest sie ohne Kontrollpuffer, sodass der Kernel einen in die andere Richtung geschickten
  Deskriptor schließt, nie annimmt.
- Höchstens vier weitere Shells leben gleichzeitig; der Shim zählt, und der Daemon zählt noch
  einmal (`TERM_002` für die fünfte).
- Eine Shell endet, wenn ihr Prozess endet, und mit der Sandbox: PID 1 beendet jede Shell, wenn
  der Agent endet, die Shells sterben mit dem Thread, der sie geforkt hat (`PR_SET_PDEATHSIG`),
  und mit dem PID-Namespace, und der Daemon beendet ihre Terminals, wenn sein Ende des Paars
  sich schließt.
- Eine ohne das Pseudo-Terminal des Daemons gestartete Sandbox (`humanitl sandbox run`, die
  Escape-Suiten) hat keine Tür.

*Was es kostet.* Ein verweigerter `socket()`-Aufruf in einer weiteren Shell wird verweigert wie
der des Agenten, aber nicht gezählt: Der Listener des gemeldeten Tors gehört zum Filter des
Agenten, und die Filter verweigern einen zweiten. Die Shell startet außerdem mit dem Filter des
Elternprozesses darunter, der `AF_UNIX` erlaubt; der Filter des Agenten darüber verweigert es,
und die Shell startet nicht, solange diese Verweigerung nicht bewiesen ist.

*Nicht vertraulich gegenüber dem Agenten.* Eine weitere Shell läuft mit der UID des Agenten im
selben PID-Namespace, und das `/proc` der Sandbox ist nicht mit `hidepid` eingehängt. Der Agent
kann deshalb die Prozesse der Shell sehen, lesen, was die Programme der Shell von ihrem Terminal
lesen (die dort getippten Tasten), und Ausgaben in dieses Terminal schreiben, die den Menschen
durch denselben Filter erreichen wie die eigene Ausgabe des Agenten. Die Shell schützt den Rechner
vor dem Agenten, nicht die Eingaben des Menschen vor dem Agenten: Tippe nie ein Secret hinein.
Die Vorgabe-Shell startet ohne Startdateien (`bash --norc --noprofile -i`, oder `sh -i` mit
nicht gesetztem `ENV`), sodass ein `~/.bashrc` oder `~/.profile`, das der Agent im Home der
Sandbox abgelegt hat, beim Öffnen nicht läuft; ein Befehl, den der Aufrufer nach `--` nennt,
entscheidet das selbst.

### Was der Filter erlaubt und verweigert

`socket()` geht nur für eine Familie in `allow_families` (`AF_INET`, `AF_INET6`) mit einem Typ in
`allow_types` (`SOCK_STREAM`) durch. Beide Listen sind keine Einstellung, sondern eine Grundsperre in
beide Richtungen: Ein Profil, das `AF_UNIX` oder `SOCK_DGRAM` nennt, wird mit `CONFIG_003`
verweigert, und der Starter gibt genau diese Grundsperre an den Shim. Die eine geplante Ausnahme ist
das spätere Profil `browser` (M7), das `AF_UNIX` für die IPC von Chromium braucht; es ist
`SocketFloor::BrowserUnixIpc` im Code des Starters und lässt sich aus keiner Profildatei setzen,
weil eine solche Datei aus einem geklonten Repository stammen kann. Das Typ-Argument wird mit
`0xff` maskiert, sodass `SOCK_NONBLOCK` und `SOCK_CLOEXEC` durchgehen. Alles andere, `AF_UNIX`,
`AF_NETLINK`, `AF_PACKET`, `AF_VSOCK`, `SOCK_DGRAM`, `SOCK_RAW`, bekommt `EPERM`.

`socketpair()` hat keine Regel und ist erlaubt: Es kennt nur `AF_UNIX`, verbindet zwei
Deskriptoren desselben Prozessbaums und ist kein Egress (Node und Bun brauchen es für die IPC
mit Kindprozessen, `CONVENTIONS.md` 4.11; `filter_allows_socketpair` in `seccomp.rs` und ESC-1
beweisen es).

Das Programm, das der Kernel ausführt, hat zwei Teile. Zuerst kommt ein handgeschriebener Vorspann:

| Gegenstand | Bedingung | Urteil |
|---|---|---|
| jeder Syscall | Architektur ist nicht die, für die der Shim gebaut wurde | Prozess beenden |
| jeder Syscall | Nummer hat das x32-Bit `0x40000000` gesetzt | `EPERM` |
| `seccomp` | Flag `SECCOMP_FILTER_FLAG_NEW_LISTENER` gesetzt | `EPERM` |
| `socket` | Familie nicht in `allow_families` | `EPERM` (Agent: über den Listener) |
| `socket` | Typ (`arg1 & 0xff`) nicht in `allow_types` | `EPERM` (Agent: über den Listener) |

Dann verweigert das seccompiler-Programm jeden Syscall der Sperrliste mit `EPERM` und erlaubt
den Rest. Die Grundsperre, `FLOOR` in `seccomp.rs`, wird in jedem Profil verweigert, auch wenn ein
Profil einen Namen vergisst:

| Syscall | Warum er verweigert wird |
|---|---|
| `ptrace` | einen benachbarten Prozess übernehmen |
| `io_uring_setup`, `io_uring_enter`, `io_uring_register` | Syscalls am Filter vorbei einreihen |
| `process_vm_readv`, `process_vm_writev` | Speicher eines anderen Prozesses lesen und schreiben |
| `pidfd_getfd` | einen Deskriptor aus einem anderen Prozess kopieren (HUM-203) |
| `keyctl`, `add_key`, `request_key` | der Schlüsselbund des Kernels |
| `kexec_load`, `kexec_file_load` | einen neuen Kernel laden |
| `init_module`, `finit_module`, `delete_module` | Kernel-Module austauschen |
| `bpf` | BPF-Programme laden |
| `perf_event_open` | Leistungszähler öffnen |
| `userfaultfd` | Seitenfehler im User-Space bedienen, der klassische Hebel, um ein Race im Kernel zu strecken |

Die letzten acht sind die übliche Härtung, die auch das Vorgabeprofil von Docker anwendet.
`pidfd_open` bleibt erlaubt, weil asyncio, Go und libuv es abfragen, und ohne `pidfd_getfd`
öffnet ein Prozess-Deskriptor nichts, was ein `kill` nicht ohnehin erreicht.

Ein Profil darf mehr verweigern, aber nur Namen aus `SYSCALLS` in `seccomp.rs`; ein unbekannter
Name beendet den Shim mit Exit 126, statt still verworfen zu werden. Über die Grundsperre hinaus sind
das: `mount`, `umount2`, `pivot_root`, `chroot`, `setns`, `unshare`, `reboot`, `swapon`, `swapoff`,
`acct`, `settimeofday`, `clock_settime`, `clock_adjtime`, `adjtimex`, `open_by_handle_at`,
`name_to_handle_at`, `personality`, `kcmp`, `quotactl`, `mbind`, `set_mempolicy`,
`get_mempolicy`, `migrate_pages`, `move_pages`, `fsopen`, `fsconfig`, `fsmount`, `fspick`,
`move_mount`, `open_tree`, `mount_setattr`, `vhangup`, `pidfd_open` und `socket` selbst. Das
mitgelieferte Vorgabeprofil verweigert genau die Grundsperre.

Maßgeblich ist die Quelle, nicht diese Seite: `humanitl-shim --rules` gibt die Regeltabelle aus,
die die aktuelle Umgebung erzeugen würde, und der Test `rule_table_lists_every_rule` gibt jede
Zeile aus. Der Test `security_doc_mentions_syscalls` schlägt fehl, wenn ein Syscall-Name aus
`seccomp.rs` auf dieser Seite fehlt.

**Prüfung (in der Sandbox).**

```sh
grep Seccomp /proc/self/status                       # "Seccomp: 2" (filter mode)
grep NoNewPrivs /proc/self/status                    # "NoNewPrivs: 1"
cat /proc/1/comm                                     # "humanitl-shim": PID 1 is the shim parent
grep Seccomp /proc/1/status                          # "Seccomp: 2"
stat -c %u /proc/1/mem; id -u                        # different: PID 1 is not dumpable
python3 -c 'import os; os.open("/proc/1/mem", os.O_RDWR)'                 # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_UNIX)'                 # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_DGRAM)'  # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_STREAM)' # works
python3 -c 'import socket; socket.create_connection(("10.0.0.1",80),2)'   # ENETUNREACH
```

**Automatisiert.** ESC-1 deckt genau diese Proben ab, einschließlich `io_uring_setup`, eines
x32-Syscalls und der Proben an PID 1 und am Elternprozess des Shims (`pid1_*`, `shim_parent_*`,
`pidfd_getfd_self`; mit `ptrace_scope` 1 oder höher verweigert Yama `mem` und `pidfd_getfd`
schon selbst, diese Fälle sind dort `skip`, und `pid1_not_dumpable` trägt die Aussage). Zur
Laufzeit `IsolationCheck::SeccompActive`: Der Shim schreibt Prüfzeilen aus der laufenden Sandbox
auf seinen eigenen Deskriptor (`HUMANITL_REPORT_FD`), der Starter liest sie und macht daraus die
drei Ergebnisse. Der Beweis kommt aus der Sandbox, in der der Agent wirklich läuft, nicht aus
einer zweiten, die zum Testen gestartet wurde.

**Was ein Angreifer versuchen würde.** Ein setuid-Programm ausführen, um den Filter loszuwerden:
`NO_NEW_PRIVS` verhindert es. Einen Nachbarn mit `ptrace` übernehmen: `EPERM`. Syscalls über
`io_uring` am Filter vorbei einreihen: Die drei `io_uring_*`-Aufrufe werden verweigert. Denselben
Syscall unter seiner x32-Nummer aufrufen, eine klassische Umgehung von Filtern: Der Vorspann
fängt das. Einen Thread ohne Filter starten: Den gibt es nicht, Threads erben ihn. Über
`/proc/1/mem` oder `pidfd_getfd(2)` in PID 1 schreiben, um Code ohne den Filter des Agenten
auszuführen: PID 1 ist der Shim, trägt den Filter der Bridge und ist nicht dumpbar, und
`pidfd_getfd` steht in der Grundsperre (ESC-1 `pid1_mem_rdwr`, `pid1_pidfd_getfd`). Den Listener des
Filters an sich nehmen, um die eigenen Aufrufe mit „weiter" zu beantworten
(`SECCOMP_USER_NOTIF_FLAG_CONTINUE`): Der Listener lebt nur im Elternprozess, der nicht dumpbar
ist, also werden `/proc/<pid>/fd`, `/proc/<pid>/mem` und `pidfd_getfd(2)` dem Agenten verweigert,
selbst mit derselben UID und `kernel.yama.ptrace_scope = 0`
(`the_agent_cannot_reach_the_descriptors_of_the_parent` in
`daemon/bin/humanitl-shim/tests/shim.rs`). Der Elternprozess antwortet nie etwas anderes als
`EPERM`, und stirbt er, antwortet der Kernel `ENOSYS`: Die Tür bleibt zu, was auch immer mit dem
Listener geschieht. Einen eigenen Listener für einen neueren Filter bekommen: Beide Filter
verweigern `seccomp()` mit `SECCOMP_FILTER_FLAG_NEW_LISTENER`, und der Elternprozess lässt seinen
Listener nie los, auch nicht nach Fehlern, die der Agent herbeiführt
(`a_dropped_listener_does_not_free_the_agent_to_take_one` in `seccomp.rs`, ESC-1
`seccomp_no_own_listener`). Den Elternprozess kapern, der die Bridge hält: siehe K-04 in
[`THREAT-MODEL.md`](THREAT-MODEL.md#k-04-tcp-sockets-allowed-by-seccomp); damit ist nichts
gewonnen, denn er trägt ebenfalls einen Filter, und die Bridge kennt genau ein Ziel, den
aufzeichnenden Proxy.

### Was sichtbar wird und was nicht

Blockiert werden heißt nicht gesehen werden. Was an der einen Tür vorbei will, ein `ssh` an eine
nackte Adresse, ein UDP-Paket, `git://`, ein Unix-Socket, erreicht den Proxy nie: Es wird kein
Flow und steht weder in der Warteschlange noch im Verlauf. Damit das Fenster trotzdem zeigt, was
der Agent **versucht** hat, zählt der Shim, was der Filter verweigert (HUM-138):

- **Sichtbar:** jeder verweigerte `socket()`-Aufruf des Agenten und seiner Kindprozesse, gezählt
  pro Paar aus Familie und Typ, mit Anzahl, erstem und letztem Zeitpunkt und welche Hälfte des
  Tors verweigert hat (Familie oder Typ). Der Shim schreibt den Stand höchstens alle zwei
  Sekunden und einmal, nachdem der Agent endet; der Daemon gibt ihn als
  `SandboxEvent.refusals` weiter, das Fenster zeigt ihn im Isolations-Reiter,
  `humanitl sandbox run` nach dem Lauf. Gezählt, nicht protokolliert: höchstens sechzehn Paare
  plus ein Sammelfach für den Rest, sodass ein Agent in einer Schleife keine Flut erzeugt. Die
  Antwort kostet einen Kontextwechsel mehr, gemessen bei etwa 5 bis 30 µs pro verweigertem
  Aufruf; erlaubte Aufrufe erreichen den Listener nie.
- **Nicht sichtbar:** ein `connect()` zu einer Adresse draußen. Er ist kein verweigerter Aufruf
  (das `socket(AF_INET, SOCK_STREAM)` davor ist erlaubt), sondern scheitert an der leeren
  Routing-Tabelle mit `ENETUNREACH`. Aufrufe, die der Kernel ohne Nachfrage verweigert, werden
  ebenfalls nicht gezählt: die Sperrliste und jeder x32-Aufruf.
- **Ein Bericht, keine Garantie.** Ein Angreifer (b), der seine Versuche verbergen will, kann
  einen weiteren eigenen Filter laden, der `socket()` selbst mit `EPERM` beantwortet; der Kernel
  nimmt diese Antwort, und der Listener hört nichts. Der Aufruf wird trotzdem verweigert. Keine
  der drei Garantien hängt an einer Zeile dieses Berichts.

Automatisiert: `esc-1/refusals_reported` (`tests/escape/refusals-proof.sh`) lässt eine Sandbox
dreimal `socket(AF_UNIX)` und zweimal `socket(AF_INET, SOCK_DGRAM)` versuchen und verlangt genau
diese Zahlen im Bericht.

### Das Fenster zwischen `exec` und `SIGKILL`

Der Shim setzt nichts durch. Er schreibt seine fünf `CHECK`-Zeilen und startet direkt danach den
Agenten; die Durchsetzung geschieht auf dem Rechner (`CONVENTIONS.md` 4.12). Der Rechner liest den
Bericht deshalb, wenn der Agent schon läuft, und hält die Sandbox danach an. Dazwischen liegt
ein Fenster, in dem die Bridge steht, der Proxy annimmt und eine `allow`-Regel ohne Menschen
weiterleitet.

Normalerweise sind das Millisekunden. Die **Obergrenze** ist die Summe der Fristen:

| Weg | Bericht fehlt nach | Anhalten | Grenze |
|---|---|---|---|
| Daemon (`SandboxService`) | `REPORT_TIMEOUT` 5 s | `SIGKILL` sofort, `KILL_GRACE` 5 s zum Einsammeln | **10 s** |
| `humanitl sandbox run`, `escape-launch` | `REPORT_TIMEOUT` 5 s | `SIGTERM`, `KILL_GRACE` 5 s, dann `SIGKILL`, `KILL_GRACE` 5 s | **15 s** |

Kein Weg in die Sandbox ist ausgenommen, und die Escape-Tests sagen über dieses Fenster nichts:
Sie messen von innen. Es zu schließen hieße, dass der Proxy Verbindungen verweigert, solange die
Isolation der laufenden Session nicht bewiesen ist; das ist ein eigenes Issue und nicht gebaut.
Bis dahin gilt: Eine rote Isolationsprüfung **hält** die Session **an**, sie **verhindert** sie
nicht (K-15 in [`THREAT-MODEL.md`](THREAT-MODEL.md#k-15-the-window-between-exec-and-sigkill)).

## 5. Erklärte Seitenkanäle

Das ist der wichtigste Abschnitt. Ein Sicherheitsversprechen, das seine Ausnahmen versteckt, ist
keines. Alle Kanäle unten werden in der Anwendung angezeigt, nicht versteckt.

| Kanal | Warum es ihn gibt | Was Humanitl in 0.1 tut | Später |
|---|---|---|---|
| LLM-Durchleitung | der Agent muss Code an das Modell schicken | eine Regel genau auf Host, Port, Schema, Methoden und genaue Inferenz-Endpunkte; gestreamt, vollständig aufgezeichnet, mit Warnung nach Funden durchsucht; bernsteinfarbene Zeile in der Isolationsanzeige | Anhalten bei Funden, Ratenbegrenzung |
| `/work` (Lesen und Schreiben) | der Agent muss Dateien schreiben | nur ein Unterpfad wird gebunden; Hook-, CI- und Editor-Verzeichnisse und Secret-Dateien verdeckt; Zusammenfassung der Session mit berührten Dateien, nach außen zeigenden Symlinks und einem Secret-Scan über den Diff | Git-Operationen aus dem Fenster auf der Host-Seite |
| Terminal-Ausgabe | der Mensch liest, was der Agent schreibt | der Daemon filtert jeden Bytestrom, bevor ein Client ihn sieht (Erlaubnisliste); Banner „Ausgabe des Agenten ist nicht vertrauenswürdig" | |
| Host-Namen im Log | blockierte Anfragen bleiben sichtbar | dokumentiert; der Export enthält Hosts, Pfade, Header und Bodies im Klartext und sagt das vor dem Schreiben | optionales Pseudonymisieren der Hosts |
| Paket-Caches | wiederholte Installationen | Caches pro Projekt, nie über Vertrauensstufen hinweg geteilt | schreibgeschützte Start-Caches |

### 5.1 Die LLM-Durchleitung

*Warum es sie gibt.* Der Agent muss Code an das Modell schicken, sonst arbeitet er nicht. Würde
jede Inferenz angehalten, müsste ein Mensch mehrmals pro Minute bestätigen; das hält niemand
durch, und nach zwei Stunden klickt er alles blind weg.

*Was Humanitl tut.* Die Durchleitung ist eine Regel, kein Loch im Filter: genau ein Host, ein
Port, ein Schema, die Methoden `POST` und `GET` (die Modellliste ist ein `GET`) und eine Liste
von Endpunkten. Ein Eintrag nennt einen Endpunkt und nie eine ganze API-Fläche. Die Vorgabe von
`llm.passthrough_paths` ist `["/v1/", "/api/"]`, und Humanitl leitet diese beiden nicht als
Präfixe durch: Beide würden Aufrufe abdecken, die den Server verändern (`POST /api/pull`,
`POST /api/create`, `DELETE /api/delete`, `POST /v1/files`, `/v1/uploads`, `/v1/vector_stores`,
`/v1/fine_tuning/jobs`, `POST /v1/load_lora_adapter` von vLLM). Es erweitert sie zu den dreizehn
Endpunkten, die Inferenz ausführen oder Auskunft geben: `/v1/chat/completions`, `/v1/completions`,
`/v1/responses`, `/v1/embeddings`, `/v1/models` und `/api/chat`, `/api/generate`, `/api/embed`,
`/api/embeddings`, `/api/tags`, `/api/show`, `/api/ps`, `/api/version`. Hat `llm.endpoint` ein
Pfadpräfix (`https://host/api/v1/`), liegen die fünf OpenAI-kompatiblen Endpunkte unter dieser
Basis, und die acht Ollama-Endpunkte fallen weg, weil es die native Ollama-API nur an der
Wurzel eines Servers gibt (HUM-261).

Jeder dieser erzeugten Endpunkte passt genau, nicht als Präfix: `/v1/models` passt auf
`/v1/models` und `/v1/models?x=1`, aber nicht auf `/v1/models/delete`, `/v1/models/` oder
`/v1/models%2Fdelete`; nichts wird dekodiert, jede andere Schreibweise wird angehalten. Ein Pfad,
den der Mensch selbst in `llm.passthrough_paths` schreibt, bleibt ein Präfix und deckt alles
darunter ab. In einer Regel endet ein genauer Eintrag mit `?` (`/v1/models?`). Ein Pfad mit
einem `..`-Segment passt nie auf ein Präfix, weil nur der Server ihn auflöst; dasselbe gilt für
jede Regel, die eine Anfrage durchlässt (`allow`, `redact`) und ein Pfadmuster oder Präfixe
trägt, auch wenn er als `%2e%2e` oder `..;` getarnt ist (HUM-204). Jede andere Anfrage an
denselben Host wird normal angehalten.

Der Verkehr wird vollständig aufgezeichnet und läuft durch die Detektoren für Funde; ein Fund
erzeugt eine Warnung (`LLM_005`, mit Host und Anzahlen und nie mit dem gefundenen Wert), hält
aber nicht an. Jeder Fund bleibt mit seiner Anfrage aufgezeichnet und im Verlauf sichtbar.
Agenten schicken mit jedem Zug das ganze Gespräch erneut, deshalb kommt die Warnung nur für eine
Anfrage, die mindestens einen Fund trägt, vor dem an diesem Endpunkt (`host:port`) in diesem
Lauf noch nicht gewarnt wurde oder zuletzt vor 15 Minuten oder länger; sie sagt, wie viele neu
sind und wie viele die Anfrage insgesamt trägt (HUM-354). Der Daemon merkt sich, wovor er
gewarnt hat, als Art des Detektors plus den Werte-Hash, den der Fund ohnehin trägt, nur im
Speicher und pro Endpunkt begrenzt. Der Speicher wird geleert, sobald ein Sandbox-Lauf startet,
ein Eintrag verfällt 15 Minuten nach seiner Warnung, und ein Fingerabdruck, der verfallen oder
aus der Begrenzung gefallen ist, warnt erneut. Eine Warnung, die kein offenes Fenster erreicht
hat, wird deshalb von der nächsten Anfrage mit demselben Fund wiederholt, sobald der Eintrag
verfallen ist oder ein neuer Lauf begonnen hat; ohne eine solche Anfrage folgt keine Karte, und
der Fund bleibt nur im Verlauf seiner Anfrage sichtbar. Eine Ausnahme: Mit eingeschaltetem `hold.hard_block_checksum_secrets` blockiert die Durchleitung eine Anfrage,
die ein durch Prüfsumme bestätigtes Secret trägt (API-Schlüssel, JWT, IBAN, Kreditkarte), als
System mit `HOLD_004`, statt sie durchzulassen (HUM-159). Weil das Modell meist im lokalen
Netzwerk steht, trägt die Regel `allow_private: true`; sie ist die einzige Regel, die bewusst in
private Adressbereiche zeigt. Das Recht ist an diese eine Regel und diese eine Anfrage gebunden.
In der Isolationsanzeige ist der Kanal eine vierte, bernsteinfarbene Zeile mit dem konkreten
Endpunkt.

*Was offen bleibt.* Steht in `llm.endpoint` ein Name statt einer Adresse, entscheidet der
Resolver, wohin die Durchleitung führt, und er entscheidet für jede Anfrage neu. Wer diesen
einen Namen kontrolliert (ein per DHCP angekündigter DNS in einem fremden WLAN, ein
kompromittierter Router), kann die Durchleitung zwischen zwei Anfragen auf einen anderen Rechner
umlenken, und weil die Regel `allow_private` trägt, sind der Router selbst oder
`169.254.169.254` gültige Ziele. Abhilfe ist eine Adresse statt eines Namens oder ein fester
Eintrag unter `resolver.overrides`, der den Namen an genau eine Adresse bindet, bevor jemand
gefragt wird.

*Was der Nutzer tun sollte.* Nur einen Rechner eintragen, den er kontrolliert. Kein geteilter
Ollama-Server ohne Authentifizierung in einem Firmennetz. Die IP-Adresse bevorzugen oder den
Namen unter `resolver.overrides` festlegen. Wer keinen eigenen Rechner hat, schaltet die
Durchleitung ab und lässt auch die Inferenz anhalten: unbequem, aber ehrlich. Einmal pro Start
auf Zeile 4 der Isolationsanzeige schauen.

#### Der Zugangsschlüssel des Modells (HUM-140)

*Wo er liegt.* Im Schlüsselbund des Systems über den Secret Service (`gnome-keyring`, `kwallet`),
angesprochen mit `secret-tool` aus `libsecret-tools`, unter den Attributen `service=humanitl` und
`account=llm`. Der Schlüssel reist über `stdin` hinein und über `stdout` hinaus, nie auf der
Kommandozeile von `secret-tool`. Antwortet der Secret Service nicht (kein `secret-tool`, kein
Session-Bus, ein Fehler oder 20 Sekunden ohne Antwort), liegt er in
`$XDG_DATA_HOME/humanitl/keys/llm.key` mit `0600` in einem Verzeichnis mit `0700`; das meldet
`CONFIG_019`, und das Formular sagt, welcher der beiden Fälle gilt. Die Datei wird nur gelesen,
wenn sie diesem Konto gehört, kein Symlink ist und für niemanden sonst lesbar ist (sonst
`LLM_009`). `config.toml` enthält nie den Schlüssel, nur seinen Verweis `llm.api_key_ref`
(`secret-service:humanitl/llm`, `keychain:humanitl/llm` oder `file:llm`). Gesetzt wird er über den RPC `LlmKey`, aus dem
Formular oder mit `humanitl llm key set`, das ihn von `stdin` liest; keine Antwort des Daemons
trägt ihn zurück.

*Unter macOS (HUM-349).* Der Schlüsselbund ist der Anmelde-Schlüsselbund (das Keychain),
angesprochen mit dem Systemwerkzeug `/usr/bin/security`, mit demselben Dienst `humanitl` und dem
Konto `llm`, dem Verweis `keychain:humanitl/llm` und denselben 20 Sekunden. Eine Client-Bibliothek
gibt es nicht: Die App ist nur ad-hoc signiert, ein Eintrag, dessen Zugriffsliste unserem Programm
vertraute, fragte also nach jedem Update. Ein von `security` angelegter Eintrag vertraut diesem von
Apple signierten Werkzeug, und nur ihm; kein `-A`, kein `-T`. Der Schlüssel geht über `stdin` an
`security -i`, nie auf dessen Kommandozeile, und kommt über `stdout` zurück. Weil der Exit-Code von
`security -i` nichts beweist (eine Zeile, die es nicht verarbeiten kann, kann mit 0 enden), liest
der Daemon den Eintrag zurück und vergleicht ihn, bevor er den Schlüssel als gespeichert meldet.
Ein Schlüssel, den `security` nicht unverändert tragen kann (Zeichen außerhalb des druckbaren ASCII
oder ein Befehl über 4000 Bytes), erreicht das Werkzeug nie. Nimmt das Keychain den Schlüssel nicht
an, etwa weil es in einer Sitzung ohne Bildschirm gesperrt ist, liegt der Schlüssel wie unter Linux
in der `0600`-Datei, und `CONFIG_019` sagt es, mit dem Grund. Ein Schlüssel, der schon in dieser
Datei liegt, wirkt über seinen `file:`-Verweis weiter; setzt man erneut einen Schlüssel, zieht er
ins Keychain um, und die Datei wird entfernt. Ein Eintrag im Keychain steht dem Konto genauso offen
wie der Secret Service: **Jeder Prozess dieses Kontos, der außerhalb der Sandbox läuft, kann ihn
mit `security find-generic-password -s humanitl -a llm -w` lesen**, ohne Rückfrage, weil die
Zugriffsliste diesem Werkzeug vertraut und nicht einem Aufrufer. Gegenüber dem Agenten hält das
Keychain wie der Secret Service: Das Seatbelt-Profil erreicht kein `securityd` (Mach-Dienste sind
eine Erlaubnisliste ohne ihn), und `~/Library/Keychains` liegt in `/Users`, das nicht lesbar ist.
`/usr/bin/security` selbst darf in der Sandbox gestartet werden; es findet dort nichts. Der Agent
bekommt den Schlüssel nur als Sandbox-Datei der Session.

*Mehrere gespeicherte Server (HUM-289).* `config.toml` darf mehrere Modellserver unter
`[llm.servers.<name>]` enthalten, jeden mit eigenem Endpunkt und eigenem Schlüsselverweis; der
Schlüssel eines neuen Servers liegt unter `account=<name>` (`secret-service:humanitl/<name>`),
der aus einer älteren Konfiguration übernommene Server behält `account=llm`, und keine zwei
Server teilen sich einen Eintrag. `llm.active` nennt den, der gilt, und die flachen Schlüssel
`llm.endpoint` und `llm.api_key_ref` sind seine Sicht: Nur der Origin des aktiven Servers
bekommt den gespeicherten Schlüssel, beim Start einer Session und in der Probe, und nur er ist die
Durchleitung der nächsten Session. Eine Probe, die einen gespeicherten Server nennt
(`ProbeLlmRequest.server`), schickt den Schlüssel dieses Servers, und nur, wenn die geprüfte
Adresse den Origin dieses Servers hat (Schema, Host, Port); der Schlüssel eines Servers geht nie
an den Origin eines anderen. Eine Regel gilt beim Laden und erneut bei jeder Verwendung (der
Schlüssel des Agenten beim Start, die Probe ohne Server): Ein Schlüsselverweis wirkt nur
zusammen mit dem Endpunkt desselben Eintrags. Setzt die Umgebung oder die Kommandozeile
`llm.endpoint` oder `llm.api_key_ref` über dem aktiven Server, oder kommen ohne gespeicherte
Server die beiden flachen Schlüssel aus verschiedenen Schichten, wird der gespeicherte Schlüssel
zurückgehalten (`CONFIG_021`). Gibt es gespeicherte Server und nennt `llm.active` keinen davon,
sind Endpunkt, Modelle und Schlüssel alle nicht gesetzt (`CONFIG_021`). Zwei Server, die sich
einen Verweis teilen, bekommen für keinen der beiden den Schlüssel, und nie wird ein Schlüssel
in einen geteilten Verweis geschrieben. Ein Schlüssel wird einem Server nur zugeordnet, wenn der
Eintrag nach dem Speichern des Schlüssels unverändert ist (gleicher Name, gleicher Endpunkt);
sonst wird der gespeicherte Eintrag wieder gelöscht. Ein Wechsel gilt für die nächste Session;
eine laufende Session behält den Server, den Schlüssel und die Durchleitung, mit denen sie
gestartet ist, und der aktive Server lässt sich nicht entfernen, während eine Session läuft oder startet; kein Wechsel und keine Entfernung, die einen Schlüssel löscht, geschieht, während ein Start seine Konfiguration auflöst, und Endpunkt oder Schlüsselverweis eines Servers ändern sich nie, während sein Schlüssel gespeichert wird (eine Sperre für den Daemon und die Kommandozeile). Das Entfernen eines Servers löscht seinen
Eintrag im Schlüsselbund auf Wunsch, und nur, wenn kein anderer Server darauf verweist.

*Wie er in die Sandbox kommt.* Wenn eine Session startet, holt der Daemon den Schlüssel und legt
ihn als Datei `/etc/humanitl/opencode/llm-api-key` in die Sandbox, aus einem memfd über
`--ro-bind-data`. `opencode.json` verweist mit `{file:/etc/humanitl/opencode/llm-api-key}` darauf.
Mit `agent.adapter = "pi"` legt derselbe Mechanismus ihn unter `/etc/humanitl/pi/llm-api-key` ab,
und `models.json` von pi verweist mit `"apiKey": "!cat \"$HUMANITL_PI_KEY_FILE\""` darauf, was
pi über `/bin/sh -c` ausführt; der Pfad ist eine Konstante des Adapters, nie ein eingerichteter
Wert, und nur der Pfad steht im `argv` dieses `cat` (HUM-274).
Er ist bewusst keine Umgebungsvariable: bwrap setzt die Umgebung des Agenten mit
`--setenv NAME VALUE` auf seiner eigenen Kommandozeile, die jeder Prozess desselben Nutzers in
`/proc/<pid>/cmdline` lesen kann. Fehlt der Schlüssel im Speicher, startet die Session nicht und
nennt `LLM_009` mit `humanitl llm key set`. Ein Plan (`Sandbox(Plan)`) liest den Schlüssel nie;
die Datei ist dort leer, sodass Plan und Start dieselbe Kommandozeile haben.

*Wo er nicht steht.* Nicht in `argv`, nicht in der Umgebung, nicht im Schnappschuss des Fensters,
nicht im Log: Der Typ `ApiKey` erscheint in jeder `Debug`-Form als `••••` und wird beim
Verwerfen überschrieben. In der Aufzeichnung steht er als `••••`: Bevor der Agent ihn sieht,
fügt der Start ihn den Masken der Aufzeichnung hinzu, und jeder aufgezeichnete Header, die
Spalten `content_type` und `content_encoding`, jeder aufgezeichnete Pfad mit Query, der Host und
der `Host`-Header tragen `••••` statt des Werts, auch URL-kodiert, ganz oder teilweise, in beiden
Schreibweisen der Hex-Ziffern. Bodies werden nicht verdeckt: Ihre Prüfsumme ist die Adresse des
Blobs. Ein Schlüssel kürzer als 16 Zeichen wird gar nicht gespeichert (`CONFIG_020`): Er ließe
sich weder zuverlässig verdecken noch finden.

*Wo er ein Fund ist.* Der Detektor `known-secret` sucht den Schlüssel wörtlich und URL-kodiert in
jedem Header, in der Query (roh und dekodiert) und im dekodierten Body jeder Anfrage, an jeden
Host. Jedes Vorkommen ist ein Fund `api_key:llm` der Stufe `checksum`. An genau einer Stelle ist
er kein Fund: im `Authorization`-Header einer Anfrage an Host und Port von `llm.endpoint`. Ein
Fund hält eine Anfrage nicht von selbst an; sie wird ohnehin angehalten, wenn keine Regel ihren
Host erlaubt, und mit eingeschaltetem `hold.hard_block_checksum_secrets` hält der Fund sie wegen
seiner Stufe an. Im Pfad vor `?` und im Host sucht kein Detektor; dort verdeckt nur die
Aufzeichnung.

Der Body wird so weit durchsucht, wie jeder Scan reicht: bis `limits.preview_cap_bytes` (darüber
hinaus meldet der Scan eine Lücke, `FINDINGS_002`), dekodiert nur für ein `Content-Encoding`, das
der Scanner kennt. Ein gzip-Body wird über alle Member hinweg dekodiert (RFC 1952 erlaubt mehrere
hintereinander, und das Ziel liest sie alle), unter einem gemeinsamen Budget aus
`limits.preview_cap_bytes` und `limits.max_decompress_ratio`. Bytes nach dem Ende eines
`deflate`- oder `br`-Stroms oder nach dem letzten vollständigen gzip-Member, die sich nicht als
weiteres Member lesen lassen, lassen den Scan den dekodierten Teil davor durchsuchen und den Rest
als Lücke melden (`FINDINGS_002`), statt den Body vollständig durchsucht zu nennen (HUM-247). Ein
`deflate`-Body wird zuerst mit dem zlib-Rahmen aus RFC 1950 gelesen und nur dann als roher
RFC-1951-Strom, wenn der Rahmen kaputt ist; liest sich der Rahmen vollständig, wird die rohe
Lesart unter denselben Grenzen mit ihm verglichen. Ein Body, dessen rohe Lesart von der
zlib-Lesart abweicht, wird als Lücke gemeldet (`FINDINGS_002`), wobei die zlib-Lesart durchsucht
wird, denn ein Empfänger, der nur den rohen Strom kennt, bekäme Inhalt, den der Scan nie gesehen
hat; das gilt, wenn der rohe Strom sauber endet, Bytes hinter seinem Ende hat oder mittendrin
abbricht, nachdem er schon abgewichen ist, weil ein streamender Empfänger diesen Inhalt bis dahin
ausgeliefert hat. Ein gültiger zlib-Kopf, der ein voreingestelltes Wörterbuch verlangt
(`FDICT`), ist ebenfalls eine Lücke: Der Scan hat kein Wörterbuch, durchsucht die rohe Lesart und
sagt, dass die zlib-Lesart nicht durchsucht wurde (HUM-266). Ein Schlüssel, der anders als
wörtlich oder URL-kodiert geschrieben ist (ein JSON-Escape, `+` als
Leerzeichen in einem Formular), wird nicht gefunden.

*Was offen bleibt.* Der Agent kann den Schlüssel lesen; er braucht ihn. Ein unterwanderter Agent
kann ihn deshalb in eine Anfrage an ein anderes Ziel stecken. Dort ist er ein Fund, außer im Pfad
vor `?` und im Host; angehalten wird die Anfrage nur bei einem Host ohne erlaubende Regel oder
mit `hold.hard_block_checksum_secrets`. Ein Schlüssel, der umkodiert hinausgeht (Base64,
aufgeteilt, verschlüsselt), wird nicht wörtlich gefunden. Wer denselben Nutzer kontrolliert,
liest den Schlüssel selbst mit `secret-tool lookup`, unter macOS mit
`security find-generic-password`; der Schlüsselbund schützt gegen andere
Nutzer, nicht gegen diesen. *Was zu tun ist:* einen Schlüssel mit so wenigen Rechten wie möglich
anlegen, beim Anbieter widerrufbar, und `libsecret-tools` installieren, damit er nicht in der
Datei landet; unter macOS den Anmelde-Schlüsselbund beim Setzen des Schlüssels entsperrt halten.

### 5.2 Das Projektverzeichnis `/work`

*Warum es es gibt.* Ein Agent, der nicht schreiben darf, ist ein Chatfenster.

*Was Humanitl tut.* Gebunden wird nur ein Unterpfad, nie `$HOME`. Der Ordner liegt unter `$HOME`
oder ist der, den ein Mensch in `config.toml` erklärt hat; ein gespeicherter Ordner bekommt diese
Prüfung bei jedem Start gegen den Pfad, auf den er dann aufgelöst wird, und eine Erklärung
außerhalb von `$HOME` gilt nur für den Pfad, den ihr Text wirklich nennt, sodass ein später in
ein Elternverzeichnis gelegter Link das Projekt nicht woandershin verschieben kann (HUM-294). Für
den Daemon erklärt nur seine eigene `config.toml`; eine Datei, die ein Client mit `--config`
nennt, erklärt nichts, und ihr Ordner außerhalb von `$HOME` wird mit dem Befehl verweigert, der
ihn in der Datei des Daemons erklärt (HUM-296). `sandbox.work_mode = "ro"` ist
möglich und der Vorschlag für Sessions, die nur analysieren. Pfade, über die geschriebene Dateien
später auf dem Rechner **ausgeführt** würden, werden abgedeckt (vollständige Liste in
`profiles/sandbox/default.toml`):

- als leeres `tmpfs`: `.git/hooks`, `.vscode`, `.idea`, `.fleet`, `.github/workflows`,
  `.gitlab-ci.yml.d`, `.direnv`, `.humanitl`;
- als leere, schreibgeschützte Datei: `.envrc`, `.env`, `.env.local`, `.git/config`, `.npmrc`,
  `.yarnrc`, `.yarnrc.yml`, `.pypirc`, `.gitlab-ci.yml`, `Jenkinsfile`,
  `.pre-commit-config.yaml`.

Eine Maske ist ein `--ro-bind-data` aus einem versiegelten, leeren memfd, nicht `/dev/null`: Das
Binden eines Geräts auf einen `nodev`-Mount ergibt `EACCES`. Der Agent sieht
eine leere Datei, und Schreibzugriffe enden mit `EROFS`. bwrap hängt nur über einen Mountpunkt
ein, der existiert. Fehlt eines dieser Verzeichnisse oder eine dieser Dateien im Projekt, liegt
nichts darüber, und was der Agent dort schreibt, landet im Projekt. Humanitl legt es **nicht**
an: Der Daemon schreibt nichts in das Projektverzeichnis. Stattdessen wird die Lücke benannt: Die
betroffenen Pfade stehen in der Zusammenfassung des Laufs, und sobald der Agent unter einen
davon geschrieben hat, listet `SANDBOX_025` (Warning) sie auf. Ein Profil kann eine Maske mit
`mounts.unmask` aufheben, was beim Start als `SANDBOX_020` erscheint; `/work/.envrc` und
`/work/.git/config` sind ausgenommen, ein Profil, das sie freilegen will, wird mit `CONFIG_003`
verweigert.

Am Ende eines Sandbox-Laufs zeigt Humanitl die berührten Dateien, markiert neue Symlinks, deren
Ziel außerhalb von `/work` liegt (`SANDBOX_022`), führt den Secret-Scan über den Diff aus
(`SANDBOX_023`), nennt jede Datei, die dieser Rechner von selbst ausführt, einen Git-Hook, ein
`Makefile`, eine `package.json` mit `postinstall`, eine Workflow-Datei (`SANDBOX_026`), und nennt
jede geänderte Datei, in die es nicht hineinsehen konnte (`SANDBOX_028`). Beide Schnappschüsse
durchlaufen einen Baum, in den der Agent bis eben geschrieben hat; jeder Zugriff geht durch
`openat2` mit `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`, und ein Symlink-Ziel
wird nur gelesen, nie verfolgt. Jeder Name in der Zusammenfassung kommt vom Agenten und wird vor
der Anzeige bereinigt; jede Zeile trägt einen Hash ihres echten Namens, und ein Kopierbefehl
wird nur aus einem Pfad gebaut, der sich wörtlich quoten lässt und den das Bereinigen nicht
verändert hätte. Die Zusammenfassung wird in der Aufzeichnung aufbewahrt (`session_summaries`);
`humanitl sessions summary <id>` holt sie lange nach dem Lauf. Mit `work_mode = "ro"` gibt es
keine Zusammenfassung: Unter `--ro-bind` kann sich nichts ändern.

*Was der Nutzer tun sollte.* Den Diff vor dem Commit lesen, besonders neue Dateien und alles
unter `.github/`, `Makefile`, `package.json` (`scripts`) und Build-Dateien. Fremde Repositorys
mit `--work-mode ro` starten oder als Wegwerf-Sandbox.

#### Die Wegwerf-Sandbox (HUM-350)

Die Wegwerf-Sandbox gibt es vorerst nur unter Linux: Sie braucht das Overlay von bubblewrap, und
unter macOS wird ein Wegwerf-Start mit `SANDBOX_030` verweigert, der Daemon bietet
`sandbox.disposable` nicht an, und `changes`, `apply` und `discard` antworten mit demselben Befund.

Mit `sandbox.mode = "disposable"` (`humanitl run --disposable`) schreibt der Agent nichts in den
Projektordner, während er läuft. Der Ordner ist die schreibgeschützte untere Schicht eines
Overlays unter `/work`; jede Änderung geht in eine obere Schicht, die der Daemon vor dem Start
unter `$XDG_RUNTIME_DIR/humanitl/disposable/<id>/` anlegt (Modus `0700`, auf tmpfs, also im
Arbeitsspeicher; der Start wird mit `SANDBOX_030` verweigert, wenn dieses Verzeichnis nicht auf
tmpfs liegt, wenn bubblewrap älter als 0.11 ist oder wenn das Projekt schreibgeschützt eingehängt
ist). Der geöffnete Projektordner erreicht bubblewrap als `/proc/self/fd/N`, sodass der geprüfte
Ordner die untere Schicht ist (HUM-263). Der Daemon öffnet den Projektordner einmal vor dem Start
und behält diesen Deskriptor: Die Grundlage für die Konfliktprüfung wird daran genommen, bevor die
Sandbox startet, der Starter hängt nur einen Ordner mit demselben Gerät und derselben Inode ein
(ein Umbenennen plus ein neuer Ordner unter demselben Pfad wird mit `SANDBOX_006` verweigert),
und der Diff und jeder Schreibzugriff einer Übernahme gehen durch ihn, nie wieder durch den Pfad.
Einträge von
`mounts.extra_rw` werden in diesem Modus schreibgeschützt eingehängt, sodass nichts vom Rechner
Beschreibbare die Sandbox erreicht; Masken und `tmpfs` unter `/work` liegen wie bisher auf dem
Overlay.

Das ist ein neuer Mount-Modus und ein neuer Schreibweg zurück auf den Rechner, und seine Grenzen
gehören zur Aussage:

- **Wo die obere Schicht liegt.** Die Sandbox sieht den Pfad der oberen Schicht nie: `/run`
  kommt nie vom Rechner, und bubblewrap öffnet die Overlay-Verzeichnisse, bevor es einhängt. Der
  Daemon liest die Schicht erst, nachdem die Sandbox geendet hat: bubblewrap ist eingesammelt, und
  der erste Prozess der Sandbox ist aus `/proc` verschwunden (verglichen mit seiner Startzeit,
  sodass eine wiederverwendete Prozessnummer nicht zählt). Endet PID 1 der Sandbox, beendet der
  Kernel jeden Prozess in ihrem PID-Namespace, und das Overlay lebt nur so lange wie ein Prozess
  in seinem Mount-Namespace, also schreibt nichts in die Schicht, während der Daemon sie liest.
- **Ihre Größe.** Die harte Grenze ist das tmpfs des Laufzeitverzeichnisses (logind legt es fest,
  standardmäßig 10 % des Arbeitsspeichers). Darunter misst der Daemon alle 250 ms mit `statvfs(2)`
  das Wachstum dieses Dateisystems gegen seinen Stand beim Anlegen der Schicht, vor dem Start, und
  hält die Sandbox an, wenn es um mehr als 2 GiB oder 200.000 Einträge wächst oder um die Hälfte
  dessen, was beim Start frei war, falls das weniger ist (`SANDBOX_032`); `humanitl sandbox run --disposable` führt denselben Wächter aus. Weil das Dateisystem
  gezählt wird und nicht der Baum, verbirgt ein Verzeichnis mit `chmod 000` nichts. Zwischen zwei
  Messungen kann ein Agent über die Grenze hinaus schreiben, bis zur harten Grenze; was andere
  Programme in dasselbe tmpfs schreiben, zählt mit und kann die Sandbox nur früher anhalten.
- **Das Lesen.** Der Diff durchläuft die obere Schicht mit demselben `openat2(RESOLVE_BENEATH |
  RESOLVE_NO_SYMLINKS)` wie die Zusammenfassung und folgt nie einem Symlink. Whiteouts
  (Zeichengerät `0:0`) sind Löschungen, `user.overlay.opaque = "y"` verdeckt das untere
  Verzeichnis, und unter einem undurchsichtigen Verzeichnis verdeckt auch jedes Verzeichnis das
  untere, sodass `rm -rf build && mkdir -p build/a` löscht, was in `build/a` lag; eine
  hochkopierte, aber unveränderte Datei ist keine Änderung. Dateien und Verzeichnisse mit
  `user.overlay.redirect` oder `user.overlay.metacopy`, die ein unprivilegiertes Overlay nie
  schreibt, werden aufgeführt und nie übernommen. Pfade und Diffs sind Daten des Agenten: Pfade werden bereinigt und tragen einen Hash ihres echten
  Namens, Steuer- und Bidi-Zeichen in einem Diff werden als Escape-Sequenzen angezeigt.
- **Das Übernehmen.** Geschrieben wird nur, was ein Mensch auswählt (`humanitl sandbox apply`, das
  vorher fragt). Jede ausgewählte Änderung wird vor dem ersten Schreibzugriff geprüft, und schlägt
  eine fehl, wird nichts geschrieben. Das Elternverzeichnis jeder Änderung wird mit
  `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS` geöffnet; fehlende Verzeichnisse werden einzeln mit
  `O_NOFOLLOW` angelegt, sodass ein inzwischen im Projekt aufgetauchter Symlink den
  Schreibzugriff anhält, statt hinauszuführen. Eine Datei wird unter eigenem Namen mit
  `O_CREAT | O_EXCL | O_NOFOLLOW` geschrieben und mit `renameat` an ihren Platz verschoben, das
  einen Symlink am Ziel ersetzt, statt ihm zu folgen; eine reine Änderung des Modus wird genauso
  geschrieben, als neue Inode, sodass ein Hardlink auf die alte Datei anderswo seinen Modus
  behält. Verzeichnisse werden mit `rwx` für den Eigentümer angelegt und bekommen ihren endgültigen
  Modus zuletzt, die tiefsten zuerst. Nie geschrieben werden: ein Symlink, dessen Ziel kein
  schlichter relativer Pfad im Projekt ist (absolut, mehr führende `..` als seine eigene Tiefe
  oder `..` nach einem Namen wie `sub/d/..`, das über einen anderen Symlink aufgelöst werden und
  das Projekt verlassen könnte), ein Gerät, ein FIFO oder ein Socket, setuid-, setgid- und
  Sticky-Bits (verworfen, und das wird gesagt) und ein Hardlink (jeder Name wird eine eigene
  Datei). Eine Datei oder ein Verzeichnis, das der Mensch seit dem Start im Projekt geändert hat
  (Typ, Größe, `mtime` oder Modus weichen vom Schnappschuss vor dem Start ab), ist ein Konflikt
  und wird nur geschrieben, wenn er für genau diese Änderung bestätigt ist: Die App nennt jede
  bestätigte Änderung in `ApplyChanges.overwrite_ids`, und ein Konflikt jeder anderen ausgewählten
  Änderung, auch einer, die erst nach der Auswahl auftrat, stoppt die Übernahme vor dem ersten
  Schreiben; `--overwrite` auf der Kommandozeile bestätigt jede ausgewählte Änderung (HUM-351).
  Direkt vor jedem `renameat`, `unlinkat` oder
  `mkdirat` wird das Ziel erneut mit dem verglichen, was bei der Prüfung da war, beim Löschen
  eines Verzeichnisses wird jeder Eintrag darunter verglichen, bevor er geht (ein geänderter oder
  neu angelegter Eintrag hält das Entfernen an, und ein Verzeichnis, dessen Baum sich nicht
  vollständig lesen lässt, wird verweigert, bevor irgendetwas geschrieben wird), und ein
  zurückgestellter Verzeichnismodus wird nur auf derselben Inode mit demselben Modus gesetzt wie
  beim Vormerken; eine Änderung dazwischen hält die Übernahme mit `partial` an. Ein Fenster von
  wenigen Systemaufrufen bleibt
  zwischen diesem Vergleich und dem Schreibzugriff: Humanitl sperrt den Ordner des Menschen nicht.
  Ein Entfernen, das mittendrin anhält, kann Verzeichnisse mit Modus `0700` und Verzeichnisse
  namens `.humanitl-flat-*` im Projekt hinterlassen, und der Befund sagt das.
- **Das Verwerfen.** `humanitl sandbox discard`, der Rest nach einer Übernahme und das Ende des
  Daemons entfernen die Schicht: ohne einem Symlink zu folgen, wobei Verzeichnisse ohne Rechte
  ihren Modus zurückbekommen und Bäume tiefer als 64 Ebenen abgeflacht werden, statt rekursiv
  hinabzusteigen. Eine Schicht, die ein abgestürzter Daemon hinterlassen hat, wird beim nächsten
  Start entfernt. Die Aufzeichnung bewahrt die Entscheidung (übernommen oder verworfen, und welche
  Dateien) in der Zusammenfassung des Laufs auf.

Was er nicht ändert: Anfragen, die der Agent durch den Proxy geschickt hat, sind draußen, was
auch immer mit der Schicht geschieht. Ein Wegwerf-Lauf sagt nichts über den Netzwerkverkehr des
Agenten, und er ist keine zweite Verteidigungslinie für die Masken oben; er nimmt für die Dauer
des Laufs den Schreibweg ins Projekt weg, mehr nicht.

### 5.3 Die Terminal-Ausgabe des Agenten

*Warum es sie gibt.* Der Mensch muss sehen, was der Agent tut. Diese Ausgabe kommt vom Agenten und
damit möglicherweise vom Angreifer.

*Was Humanitl tut.* Die Ausgabe läuft im Daemon durch `humanitl_core::TerminalFilter`, bevor sie
irgendeinen Client erreicht: auf dem Weg zum Terminal-Widget und auf dem Weg zu `humanitl run`, das
sie in das Terminal des Nutzers schreibt. Der Filter ist eine Erlaubnisliste mit zwei Richtlinien.

- **`ColourOnly`, der Strom ohne Pseudo-Terminal** (`humanitl run`, `SandboxEvent.output`): Von
  allen Steuersequenzen geht genau eine durch, `ESC [ … m` für Farbe und Attribute. Verworfen
  werden OSC 52 (Zwischenablage), OSC 8 (Hyperlinks, die ein fremdes Ziel hinter harmlosem Text
  verstecken), Fenstertitel, jede Cursorbewegung, jedes Löschen und Scrollen, das Zurücksetzen des
  Terminals und die String-Sequenzen DCS, SOS, PM und APC (`ESC P tmux; …` würde sonst eine
  verbotene Sequenz durch tmux schmuggeln).
- **`FullScreen`, das Terminal der Session** (`Terminal`-RPC, HUM-042): Ein Vollbild-Agent zeichnet
  mit absoluter Adressierung, deshalb lässt diese Richtlinie jede CSI-Sequenz und jede
  Escape-Sequenz außer `ESC c` durch. Sie verwirft jede String-Sequenz außer einer kurzen Liste
  von OSC-Nummern, Farbe (4, 10, 11) mit ihren Rücksetzungen (104, 110, 111) und
  Prompt-Markierungen (133), deren Nutzlast druckbares ASCII sein muss; `CSI … t` (XTWINOPS, das
  den Fenstertitel an OSC 0 vorbei setzt und zurückliest und die Fenstergröße ändert); und
  `ESC c`.

Beide Richtlinien erkennen jede Sequenz in drei Schreibweisen: eingeleitet mit `ESC`, als
einzelnes C1-Byte (`0x9b` ist CSI, `0x9d` ist OSC) und in ihrer wohlgeformten UTF-8-Kodierung
(`C2 9B`, `C2 9D`), weil VTE-basierte Terminals UTF-8 vor dem Parsen dekodieren und `U+009B` als
CSI ausführen. Eine Sequenz, die mit einem C1-Einleiter beginnt, geht nie durch. Nur wohlgeformtes
UTF-8 in kürzester Form verlässt den Daemon: Eine überlange Form wie `E0 82 9B` ist `U+009B`, und
Surrogate und Codepunkte über `U+10FFFF` werden ebenfalls verworfen. Der Ringpuffer, der einen
spät kommenden Client bedient, enthält gefilterte Bytes. Über dem Terminal-Widget sagt ein
dauerhaftes Banner, dass die Ausgabe des Agenten nicht vertrauenswürdig ist, und das Widget
registriert keinen Handler für die Zwischenablage: Es verweigert die Anfragen an die
Zwischenablage von OSC 52 (und von der iTerm2-Sequenz `CopyToClipboard`) ausdrücklich, schreibt
nichts und antwortet nichts, weil der Emulator sonst eigene Handler installieren würde, die die
Zwischenablage schreiben und lesen, während das Terminal den Fokus hat (HUM-313). Was der
Emulator beantwortet, sind Farbabfragen: OSC 4, 10 und 11 und die Abfrage des Farbschemas
`CSI ? 996 n` bekommen die Farben des Themes über den Tastaturstrom zurück, und nur diejenigen
davon, die den Filter des Daemons passiert haben, erreichen ihn. Text erreicht die Zwischenablage nur durch eine Handlung des Nutzers: `Ctrl+Shift+C`
oder das Kontextmenü; Einfügen geht mit `Ctrl+Shift+V`, über das Menü oder die mittlere
Maustaste, als Bracketed Paste eingerahmt, solange das Programm diesen Modus verlangt hat, wobei jede Escape-Sequenz
des eingefügten Texts entfernt wird.

**Das steuernde Terminal (HUM-280).** bubblewrap läuft in jeder Kommandozeile mit
`--new-session`, und das bleibt so: `setsid` vor dem Shim löst die ganze Sandbox von dem
Terminal, das bwrap gestartet hat, sodass `TIOCSTI` (das in die Eingabewarteschlange eines
Terminals schreibt und nur auf dem eigenen steuernden Terminal des Aufrufers erlaubt ist)
das Terminal des Nutzers nie erreichen kann. Eine vom Daemon gestartete Session sitzt auf einem
Pseudo-Terminal, das der Daemon für sie geöffnet hat; das Terminal des Nutzers wird nie an die
Sandbox übergeben, und der Daemon öffnet die Follower-Seite mit `O_NOCTTY`, sodass sie auch nie
das steuernde Terminal des Daemons wird. Für einen solchen Start, und nur für ihn, gibt der
Starter `--controlling-terminal` an den Shim. Der Kindprozess des Shims, der erste Prozess der
Session, ruft dann `setsid` und `TIOCSCTTY` mit Argument 0 auf Deskriptor 0 auf, vor dem
seccomp-Filter und dem `exec`. Argument 0 stiehlt nie ein Terminal, das schon steuerndes
Terminal irgendeiner Session ist, und Stehlen bräuchte `CAP_SYS_ADMIN` im ursprünglichen
User-Namespace, das die Sandbox nie hat. Das Terminal eines interaktiven Nutzers ist das
steuernde Terminal der Session der Shell, die darin läuft; ein Fehler im Starter, der den
Schalter mit geerbten Deskriptoren weitergäbe, endete in diesem üblichen Fall also mit `EPERM`,
das der Shim meldet und übersteht; die Garantie selbst ist, dass der Schalter dann nie
weitergegeben wird. Mit seinem eigenen Pseudo-Terminal als steuerndem Terminal unterbricht `Ctrl+C` in einer
interaktiven Shell das Vordergrundprogramm, und die Jobsteuerung funktioniert. `TIOCSTI` in der
Sandbox erreicht jetzt nur die eigene Eingabe dieses Pseudo-Terminals, die die Sandbox ohnehin
schreibt; die Master-Seite bleibt im Daemon (`CLOEXEC`, nie vererbt), und was wieder herauskommt,
läuft wie jedes andere Byte durch den Filter oben. Eine mit geerbten Deskriptoren gestartete
Sandbox (`humanitl sandbox run`, die Escape-Suiten) bekommt den Schalter nie.
`SandboxHandle::interrupt` signalisiert die Vordergrundgruppe des Terminals (`tcgetpgrp` auf dem
Master) zusätzlich zur Gruppe des Inits der Sandbox, die beide in der Sandbox liegen.

*Was der Filter nicht abdeckt.* Ein C1-Byte **innerhalb** eines längeren druckbaren
UTF-8-Zeichens bleibt Text; ein Terminal, das nicht in UTF-8 arbeitet, liest es trotzdem als
Steuerzeichen. Jedes solche Byte zu verwerfen hieße, keinen Text außerhalb von ASCII anzuzeigen;
das ist eine Entscheidung, keine zu schließende Lücke. Der Filter deckt auch nicht ab, was
**Text** ist: Der Agent kann `[humanitl] request allowed: …` selbst ausgeben. Deshalb steht die
Aussage, auf die es ankommt, nämlich dass eine Anfrage auf eine Entscheidung wartet, im Streifen
**über** dem Terminal, den das Fenster aus dem Ereignisstrom zeichnet, und in einem eigenen Frame
(`TerminalOutput.notice`), nie in den Bytes des Agenten. *Was der Nutzer tun sollte:* angezeigte
Befehle nie ungelesen in eine Shell des Rechners einfügen.

### 5.4 Host-Namen in der Aufzeichnung

*Warum es sie gibt.* Auch eine blockierte Anfrage wird aufgezeichnet, sonst wüsste niemand, was
der Agent versucht hat.

*Was Humanitl tut.* Die Aufzeichnung liegt lokal unter `$XDG_DATA_HOME/humanitl`; nichts wird
hochgeladen. Der Export schreibt alles, was aufgezeichnet wurde: Host, vollständigen Pfad mit
Query, alle Header und beide Bodies im Klartext, und der Export-Dialog sagt das in einem Satz,
bevor die Datei geschrieben wird. Optionales Pseudonymisieren der Hosts ist nach dem MVP geplant.
*Was der Nutzer tun sollte:* einen Export behandeln wie die Anfragen selbst; er enthält Tokens in
Headern und Secrets in Bodies. Schon Host-Namen allein können Kundenprojekte verraten.

### 5.5 Paket-Caches

Ohne Cache kostet jede Installation Netzwerkverkehr und damit eine Freigabe. Caches werden pro
Projekt gehalten, nie über Vertrauensstufen hinweg geteilt; wer keine einrichtet, bekommt keine.
Teile keinen Cache zwischen einem Kundenprojekt und einem Experiment. Schreibgeschützte
Start-Caches sind nach dem MVP geplant.

### 5.6 Weitere bekannte Grenzen

Eine ehrliche Liste dessen, was in 0.1 fehlt oder schwächer ist, als man annehmen könnte:

1. **Keine externe Verankerung des Audit-Logs.** Die Anker liegen in der Datei und in SQLite auf
   demselben Rechner ([Abschnitt 8](#8-the-audit-log)).
2. **Kein HTTP/2 zum Upstream.** Der Proxy spricht auf beiden Seiten HTTP/1.1; ALPN bietet nur
   `http/1.1` an. gRPC über TLS scheitert sichtbar mit `PROXY_007`, nicht mit einem stillen
   Rückfall.
3. **WebSocket-Frames werden nicht angehalten.** Nach einem freigegebenen Upgrade fließen Frames
   und werden aufgezeichnet, aber nicht einzeln bestätigt. Wer das nicht will, blockiert das
   Upgrade.
4. **Antworten werden nicht angehalten.** Das MVP moderiert Anfragen. Antworten werden gestreamt
   und aufgezeichnet; ein bösartiger Server kann Inhalt liefern, der als Prompt-Injection wirkt.
   Deshalb ist die ausgehende Richtung streng.
5. **Keine wiederholte Isolationsprüfung.** Die drei Prüfungen laufen beim Start; eine
   regelmäßige Wiederholung kommt nach dem MVP.
6. **Das Fenster zwischen `exec` und `SIGKILL`** ([Abschnitt 4](#the-window-between-exec-and-sigkill)).
7. **Pseudonyme in Antworten.** Die Zuordnung der Pseudonyme lebt pro Session im Speicher; das
   Zurücktauschen der Pseudonyme in Antworten kommt nach dem MVP.
8. **Eine Sandbox-Technik.** bubblewrap ist in 0.1 das einzige Backend. Weitere Backends sind
   hinter `SandboxBackend` geplant und nur zulässig, wenn `tests/escape/` gegen sie besteht.
9. **Kein `ProtectHome` in der systemd-Unit** ([Abschnitt 6](#service-hardening)).
10. **Verweigerte Versuche stehen nicht im Verlauf** (HUM-138). Der Isolations-Reiter und
    `humanitl sandbox run` zeigen sie, solange die Session läuft; aufgezeichnet werden sie noch
    nicht.
11. **Der `/tmp`-Ausweichweg des Laufzeitverzeichnisses** (HUM-212). Ohne `XDG_RUNTIME_DIR` und
    ohne `/run/user/<uid>` legen Daemon und Clients Socket und Token unter
    `$TMPDIR/humanitl-<uid>` ab. Ein anderes Konto kann dieses Verzeichnis zuerst anlegen oder
    dort einen Symlink ablegen. Beides wird verweigert: Der Daemon übernimmt ein bestehendes
    Verzeichnis nur, wenn es kein Symlink ist und ihm gehört (`DAEMON_004`), und Clients lesen das
    Token nur, wenn Verzeichnis und Token-Datei dem eigenen Konto gehören und für Gruppe und
    andere geschlossen sind (`DAEMON_001`). Ein Fremder kann den Start trotzdem verhindern,
    solange sein Verzeichnis da ist. Abhilfe ist ein eigenes `XDG_RUNTIME_DIR` unter dem
    Home-Verzeichnis für die ganze Sitzung ([`INSTALL.md`](INSTALL.md)). Es ist eine Schwäche
    der Verfügbarkeit, keine der Vertraulichkeit.
12. **Ein Ausfall der Aufzeichnung hält die Aufzeichnung an, nicht den Verkehr** (HUM-250). Ist
    die Platte voll oder die Datenbank gesperrt, gehen freigegebene Anfragen weiter hinaus, werden
    aber nicht aufgezeichnet. Der Daemon schickt `RECORDER_001` oder `RECORDER_003` an jeden
    Client, einmal pro Ursache und erneut an einen Client, der sich während des Ausfalls
    verbindet; die Statusleiste wird rot, jede neue Karte sagt, dass sie nicht aufgezeichnet
    wird, und die erste Freigabe der Session wartet auf eine Bestätigung. `RECORDER_005` sagt,
    wann die Aufzeichnung wieder funktioniert. Verlorene Flows werden nicht nachträglich
    geschrieben.

### Außerhalb des Umfangs

Humanitl behauptet nicht, zu schützen gegen: einen Angreifer mit root auf dem Rechner oder einen
Kernel-Exploit; physischen Zugriff; manipulierte Humanitl-Pakete; den Menschen, der freigibt; den
LLM-Host; was der Agent nach `/work` schreibt und ein Mensch später weitergibt;
mikroarchitektonische Seitenkanäle und Timing-Seitenkanäle; Verfügbarkeit (ein blockierter Agent
ist das beabsichtigte Ergebnis); andere Prozesse desselben Kontos; Netzwerkangriffe auf den Rechner
selbst. Einzelheiten in
[`THREAT-MODEL.md`](THREAT-MODEL.md#out-of-scope).

## 6. Der Proxy

**Eigene CA pro Installation.** Um den Inhalt von HTTPS-Anfragen zu zeigen, muss Humanitl TLS
terminieren („MITM"). Der Daemon legt beim ersten Start seine eigene Zertifizierungsstelle an.
Der private Schlüssel liegt unter `$XDG_DATA_HOME/humanitl/ca/ca.key` mit Modus `0600`, neben
`ca.crt`. Nur das öffentliche Zertifikat kommt in die Sandbox, als `/etc/humanitl/ca.crt`, und
das Umgebungs-Kit lässt `SSL_CERT_FILE`, `CURL_CA_BUNDLE`, `REQUESTS_CA_BUNDLE`,
`NODE_EXTRA_CA_CERTS`, `GIT_SSL_CAINFO`, `CARGO_HTTP_CAINFO`, `PIP_CERT`, `NPM_CONFIG_CAFILE` und
die anderen Variablen des Profils darauf zeigen.

**Nie im Vertrauensspeicher des Rechners.** Das Zertifikat wird nicht in den Vertrauensspeicher des
Systems oder des Browsers aufgenommen. Eine CA dort hieße, dass jeder, der an den Schlüssel kommt,
jede TLS-Verbindung des Nutzers fälschen kann, auch das Online-Banking, das mit Humanitl nichts zu
tun hat. Das Vertrauen endet an der Grenze der Sandbox: Wer `ca.crt` hat, kann nichts fälschen,
und wer `ca.key` hat, hat den Rechner schon. *Prüfung:* `ls -l $XDG_DATA_HOME/humanitl/ca/` zeigt
`0600` an `ca.key`; `trust list | grep -i humanitl` (oder `ls /usr/local/share/ca-certificates/`)
findet nichts.

**Vertrauen gegenüber Zielen: der Zertifikatsspeicher des Systems (HUM-257).** Die andere Richtung
ist, wem der Daemon glaubt, wenn er sich selbst mit einem Ziel verbindet: im Weiterleitungsweg des
Proxys und in der Endpunkt-Probe hinter `ProbeLlm`. Standardmäßig sind das die Wurzeln des
Systemspeichers (`/etc/ssl/certs`, gepflegt mit `update-ca-certificates`), gelesen mit
`rustls-native-certs`, genauso wie für `curl` und den Browser, sodass eine Firmen-CA für einen
Modellserver im Firmennetz funktioniert.

- *Abwahl.* `tls.trust_system_store = false` (Schalter „Zertifikate des Systems" in den
  Einstellungen oder `humanitl config set tls.trust_system_store false`, wirksam nach einem
  Neustart des Daemons) nutzt nur `webpki-roots`. Der Schlüssel ist im Projekt-Profil gesperrt.
- *Rückfall.* Liefert der Speicher keine brauchbare Wurzel, nimmt der Daemon `webpki-roots`,
  statt jede Verbindung scheitern zu lassen; ein einzelnes Zertifikat, das rustls zurückweist,
  kostet eine Warnung im Log. Das Startlog nennt einmal, wie viele Wurzeln woher kamen
  (`upstream trust anchors loaded`, mit `anchors`, `source`, `skipped` und `env`).
- *Was ein Angreifer gewinnt.* Wer eine CA in den Systemspeicher legen kann, ist root, und root
  liegt außerhalb des Modells. Ein Prozess desselben Kontos außerhalb der Sandbox könnte den
  Daemon über `SSL_CERT_FILE` oder `SSL_CERT_DIR` auf seinen eigenen Speicher lenken; er könnte
  ebenso gut `config.toml` ändern, und wer den Daemon konfiguriert, ist nicht abgedeckt. **Diese
  Variablen dürfen nie auf einen Pfad zeigen, den die Sandbox beschreiben kann**, etwa das
  Projektverzeichnis. Der Daemon warnt beim Start, wenn ein solcher Pfad für andere beschreibbar
  ist oder dem Konto gehört, und die Startzeile nennt die Umlenkung in `env`.
- *Was sich nicht ändert.* Die Sandbox bekommt weiterhin nur `ca.crt`, und die Entscheidung über
  eine Anfrage fällt vor jeder Verbindung; eine zusätzlich vertraute CA öffnet kein Ziel, das
  keine Regel und kein Mensch freigegeben hat.

**Die Test-Wurzel und ihr Flag (`--allow-test-ca`).** Bei Testläufen liegt das Ziel oft auf
demselben Rechner mit einem selbstsignierten Zertifikat. `humanitld --allow-test-ca` liest die
Datei in `resolver.test_ca` als PEM und fügt jedes Zertifikat darin als Vertrauensanker für die
Verbindungen des Proxys (und von `ProbeLlm`) zu Zielen hinzu. Der Pfad muss absolut sein (sonst
`CONFIG_012`, bevor die Datei angefasst wird); mit dem Flag und einer unbrauchbaren Datei startet
der Daemon nicht (`CONFIG_010`); der Schlüssel ohne das Flag oder das Flag ohne den Schlüssel
vertraut nichts und warnt (`CONFIG_011`). Es ist ein Flag und kein Schlüssel, weil die
Konfiguration aus Dateien kommt, die ein Projekt mitbringen kann, und die Kommandozeile dem
Menschen gehört, der den Daemon startet. Das Flag steht in keiner systemd-Unit und keinem
Startbefehl eines Pakets. Es ist in `ps -o args= -p <pid>` und als `WARN`-Zeile beim Start
sichtbar; über die Schnittstelle des Daemons ist es nicht sichtbar (`GetInfo` trägt dafür noch
keine Fähigkeit). Einen zweiten Hebel dieser Art gibt es nicht: `experimental.upstream_port_map`
wurde entfernt (HUM-088), weil ein stilles Umleiten von 443 auf einen anderen Port verschieben
würde, was ein Mensch freigegeben hat.

**Certificate Pinning.** Programme, die nur ein eingebautes Zertifikat akzeptieren, scheitern
sichtbar mit einem TLS-Fehler. Humanitl bietet **keinen** Rückfall, der die Verbindung
unverschlüsselt durchleitet: Das wäre ein Tunnel für beliebige Daten, und die Aufzeichnung wäre
eine Lüge.

**Grenzen und Zeitüberschreitungen.** Der Body der Anfrage wird vollständig gepuffert, bevor der
Mensch gefragt wird; wer nur Header zeigt und den Body streamt, versteckt genau den Teil, in dem
Daten hinausgehen. Die vollständige Liste mit Codes steht in [`reference/limits.md`](reference/limits.md):

| Grenze | Vorgabe | Bei Überschreitung |
|---|---|---|
| `limits.hold_body_cap_bytes` | 32 MiB | `413`, `BlockReason::BodyCap` und `LIMIT_001` mit dem Vorschlag einer Regel `ask` + `stream: true`, auch wenn eine Regel den Host erlaubt; außer bei einer passenden `allow`- oder `ask`-Regel mit `stream: true` |
| `limits.preview_cap_bytes` | 8 MiB | die Vorschau wird gekürzt; die Entscheidung bleibt möglich |
| `limits.max_decompress_ratio` | 100 | die Dekompression bricht beim Dekodieren ab (Zip-Bombe), `LIMIT_003` |
| `limits.hold_max_bytes` | 256 MiB | Summe aller Bodies in Bearbeitung, beim Puffern reserviert; eine Anfrage, die nicht mehr passt, bekommt `503`, `BlockReason::HoldMemory`, `LIMIT_002`; angehaltene Anfragen werden nie verworfen |
| `limits.hold_max_flows` | 200 | eine neue Anfrage bekommt `503`, `BlockReason::HoldMaxFlows`, `LIMIT_002` |
| `hold.timeout_secs` | 300 | `504`, `BlockReason::Timeout`: Ablauf heißt **blockieren**, nie erlauben |
| `limits.connect_timeout_secs` | 10 s | der Verbindungsaufbau zum Ziel bricht ab, der Flow ist `Failed` |
| `limits.header_timeout_secs` | 30 s | Kopf der Client-Anfrage und die Pause vor der nächsten Anfrage; außerdem der TLS-Handshake nach `CONNECT` und, bei einem gestreamten Upload, die Zeit vom Ende des Bodys bis zu den Antwort-Headern (`LIMIT_004`) |
| `limits.body_timeout_secs` | 300 s | Stille **zwischen zwei Stücken** eines Bodys, in beiden Richtungen: Body der Anfrage `408` mit `PROXY_011`; gestreamte Antwort abgeschnitten und als gekürzt markiert (`LIMIT_004`); nie eine Grenze für die Gesamtdauer |
| `limits.max_client_connections` | 256 | die Accept-Schleife antwortet `503` mit `Connection: close`; `PROXY_010`, einmal pro Minute zusammengefasst |

Die Zeitüberschreitungen messen absichtlich Stille, nicht Langsamkeit: Eine Gesamtdauer von 300
Sekunden würde den Strom des Modells, den erklärten Seitenkanal, und jeden langen Download
abschneiden. Ein langsamer, aber nie stiller Antwortstrom wird nur durch die Zahl der Verbindungen
begrenzt; eine Bandbreitengrenze gibt es noch nicht, und eine Gesamtfrist für das Puffern ist
HUM-258. `Expect: 100-continue` beantwortet der Proxy selbst; vor der Entscheidung erreicht kein
Byte den Upstream. Ein abgeschnittener Antwort-Body bleibt in jedem Export erkennbar
(`response_body_truncated`).

**`stream: true`: die eine Ausnahme vom Puffern (HUM-057).** Eine Anfrage über
`limits.hold_body_cap_bytes` lässt sich nicht wie jede andere anhalten. Sie wird blockiert, außer
eine Regel passt, die `stream: true` trägt und `allow` oder `ask` sagt, **und**
`hold.hard_block_checksum_secrets` ist aus. Dann fällt die Entscheidung am Kopf: `allow` lässt
ohne Nachfrage durch, `ask` hält an, und der Mensch sieht Methode, Ziel und Header, aber keinen
Body; erst die Freigabe liest den Rest der Leitung. Kopf, Query und Header werden durchsucht; der
Body geht **undurchsucht** hinaus, mit `LIMIT_001` als `Info` am Flow, und wird wie eine
gestreamte Antwort bis `limits.recorder_max_body_bytes` aufgezeichnet. Eine `block`- oder
`redact`-Regel mit `stream: true` streamt nicht. Der Vorschlag, den `LIMIT_001` bei einer
Blockierung macht, ist eine `ask`-Regel mit `stream: true`, nie `allow`. Mit eingeschalteter
harter Sperre greift `stream: true` nie, und `LIMIT_001` schlägt stattdessen vor, die Grenze
anzuheben.

### Härtung des Dienstes

Die systemd-Unit (`packaging/systemd/humanitld.service`) läuft als Nutzerdienst, nie als
System-Unit, und ist gehärtet, so weit das Sandbox-Backend es zulässt (HUM-053): `NoNewPrivileges`,
`PrivateTmp`, `ProtectSystem=strict` mit `ReadWritePaths` für die Daten-, Konfigurations- und
Laufzeitverzeichnisse, `PrivateIPC`, `PrivateMounts`, `PrivateUsers`, `ProtectClock`,
`ProtectControlGroups`, `ProtectKernelModules`, `ProtectProc=invisible`, `KeyringMode=private`,
`LockPersonality`, `RestrictRealtime`, `RestrictFileSystems=~@privileged-api`,
`CapabilityBoundingSet=CAP_SYS_ADMIN`, `SystemCallFilter=@system-service @mount @sandbox sethostname`
und `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK`. Jeder Kindprozess des Daemons
erbt diese Einschränkungen, die Sandbox und der Agent eingeschlossen; sie ergänzen die drei
Garantien und ersetzen keine davon.

Bewusst **nicht** gesetzt, jeweils gemessen: `RestrictNamespaces` (die Sandbox braucht
Namespaces), `ProtectKernelTunables`, `ProtectKernelLogs` und `ProtectHostname` (die Sandbox
könnte ihr eigenes `/proc` nicht mehr einhängen), `RestrictSUIDSGID`, `MemoryDenyWriteExecute`
(vom Agenten geerbt; node und jede JIT-Laufzeit stirbt beim Start),
`SystemCallArchitectures=native` (der Shim stirbt, wenn er seinen eigenen Filter prüft),
`PrivateDevices` (kein Terminal für den Agenten) und `ProtectHome`.
`ProtectHome=read-only` passt nicht zu Projekten unter `$HOME`, die beschreibbar eingehängt
werden; der Daemon läuft ohnehin als derselbe Nutzer. `ProtectSystem=strict` macht alles außerhalb
von `$HOME` und dem Laufzeitverzeichnis schreibgeschützt: Ein Projekt außerhalb von `$HOME` ist in
der Sandbox schreibgeschützt, bis der Mensch eine eigene `ReadWritePaths`-Zeile hinzufügt
([`INSTALL.md`](INSTALL.md)).

**Socket-Aktivierung.** Das Paket liefert `humanitld.socket` mit: systemd bindet
`$XDG_RUNTIME_DIR/humanitl/daemon.sock` mit Modus `0600` in einem Verzeichnis mit `0700` und
übergibt den Deskriptor an den Daemon. Der Daemon nimmt ihn nur, wenn `LISTEN_PID` ihn selbst
nennt, genau ein Socket übergeben wurde und es ein lauschender Unix-Stream-Socket an genau dem
Pfad ist, den die Clients suchen; sonst startet er nicht (`DAEMON_013`). Er entfernt
`LISTEN_PID`, `LISTEN_FDS` und `LISTEN_FDNAMES` vor dem ersten Thread aus seiner Umgebung, und
weder das Sandbox-Backend noch der Agent erbt den lauschenden Socket. Das Token schreibt weiterhin
der Daemon selbst, und ohne das Token nimmt der Dienst keinen Aufruf an.

**Fuzzing.** Sechs cargo-fuzz-Ziele laufen jede Nacht je zehn Minuten
(`.github/workflows/fuzz-nightly.yml`, HUM-056): der HTTP-Anfragepfad, der Body-Decoder (gzip,
deflate, brotli), der Chunked-Decoder, der YAML-Parser für Regeln, die Filtersprache für Flows und
der Audit-Verifizierer.

## 7. Die Regel-Engine

Regeln sind eine geordnete Liste; die erste passende gewinnt; die Vorgabe ist `ask`.
Session-Regeln (im Speicher, `expires: session`) werden vor dauerhaften ausgewertet, sodass eine
gerade getroffene Entscheidung sofort gilt. Die Feldreferenz ist [`reference/rules.md`](reference/rules.md).
Die Fallen sind alle bekannt und alle getestet (ESC-4):

- **Normalisierung vor dem Vergleich.** Host-Namen werden mit `idna::domain_to_ascii` in A-Labels
  (Punycode) umgewandelt, kleingeschrieben und von einem abschließenden Punkt befreit.
  `API.GITHUB.COM.` passt auf `*.github.com`.
- **Globs arbeiten auf Labels, nicht auf Zeichen.** `*` ist genau ein Label, `**` eines oder
  mehrere. `*.github.com` passt auf `api.github.com`, aber **nicht** auf `github.com` selbst und
  **nicht** auf `a.b.github.com`. Ein Vergleich auf Teilzeichenketten wäre das klassische Loch:
  `evil-github.com` und `github.com.evil.io` passen nie.
- **Internationalisierte Namen sehen aus wie andere.** `xn--80ak6aa92e.com` erscheint in vielen
  Schriften als `аpple.com` mit einem kyrillischen „а". Das Fenster zeigt immer die A-Label-Form
  und daneben die Identität aus dem mitgelieferten Katalog.
- **IP-Literale passen nie auf einen Host-Glob.** Sie brauchen eine ausdrückliche Regel mit
  `host: "ip:192.168.1.50"` oder `host: "cidr:192.168.0.0/16"`.
- **Private Bereiche werden verweigert.** Löst ein Name zu RFC 1918, `127/8`, `169.254/16`
  (einschließlich `169.254.169.254`, der Metadaten-Adresse der Clouds), `100.64/10`, `0/8`,
  `192.0.0/24`, `198.18/15`, `240/4`, `::`, `::1`, `fc00::/7`, `fe80::/10` oder `fec0::/10` auf,
  wird die Verbindung verweigert, außer die passende Regel trägt `allow_private: true`. Dasselbe
  gilt für IPv6-Adressen, die eine solche IPv4-Adresse in einem festen Muster einbetten
  (IPv4-mapped, IPv4-compatible, IPv4-translated, NAT64 `64:ff9b::/96`, 6to4 `2002::/16`, ISATAP),
  und für Teredo (`2001::/32`) und lokales NAT64 (`64:ff9b:1::/48`), deren eingebettete Adresse
  sich nicht eindeutig lesen lässt. 6rd hat kein festes Präfix und wird nicht erkannt.
- **Übereinstimmung der Authority.** Nach der TLS-Terminierung wird der `Host`-Header oder die
  `:authority` jeder Anfrage mit dem `CONNECT`-Ziel und der SNI verglichen. Eine Abweichung wird
  ohne Nachfrage blockiert (`BlockReason::AuthorityMismatch`, `403`): die Abwehr gegen Domain
  Fronting.
- **DNS nach dem Erlauben.** Siehe unten.
- **WebSocket.** Ein Upgrade passt nur auf eine Regel, die `upgrade: websocket` trägt; sonst
  `ask`. Das Fenster sagt vor der Freigabe, dass Frames aufgezeichnet, nicht angehalten werden.
- **Weiterleitungen erben nichts.** Ein `301` oder `302` lässt den Client eine neue Anfrage an ein
  neues Ziel schicken, die erneut durch die Regeln läuft.
- **Unbekannte Methoden führen zu `ask`**, nie zu `allow`.
- **Keine Regel hebt die harte Sperre auf.** Mit eingeschaltetem
  `hold.hard_block_checksum_secrets` geht eine Anfrage, die ein durch Prüfsumme bestätigtes Secret
  trägt, nicht unverändert hinaus: Eine `allow`-Regel und die LLM-Durchleitung blockieren sie als
  System (`BlockReason::Secret`, `403`); ohne Regel wird sie angehalten, und `Decide(Allow)` endet
  mit `HOLD_004`. Sie geht nur bearbeitet hinaus, und nur, wenn ein zweiter Scan der bearbeiteten
  Fassung kein solches Secret mehr findet (HUM-049, HUM-159). Die Prüfung sitzt im Daemon, nicht
  im Fenster.

**DNS nach dem Erlauben (ADR-006).** Ein Host-Name trägt bis zu 63 Byte pro Label und 253
insgesamt. Wer `ZGllcyBpc3QgZWluIGdlaGVpbW5pcw.attacker.example` auflösen lässt, hat die Daten
übertragen, sobald irgendein Resolver fragt: Der autoritative Server des Angreifers sieht die
Abfrage, auch wenn keine HTTP-Verbindung folgt. Ein Proxy, der auflöst, **bevor** ein Mensch
entschieden hat, ist selbst der Ausleitungskanal. Humanitl hält Anfragen deshalb am Host-Namen als
Zeichenkette an. Es löst erst nach `allow` auf, genau einmal, über den Port `Resolver`, nie über
den System-Resolver eines HTTP-Connectors; hinter dem Port antwortet der Namensdienst des Systems
oder, wenn `resolver.nameserver` gesetzt ist, fragt hickory genau diesen Server. Die gefundene
Adresse wird an die Verbindung geheftet (`Egress::connect(authority, Some(ip))`), sodass eine
zweite Abfrage kein anderes Ziel liefern kann (DNS-Rebinding). Die Domain-Vorschau im Fenster holt
nichts aus dem Netzwerk; sie kommt aus einem mitgelieferten Katalog.

*Prüfung.* `humanitl rules test https://api.github.com/repos/x/y` gibt die passende Regel aus und
endet mit 0 (allow), 10 (block) oder 11 (ask). ESC-4 führt die vollständige Tabelle aus. ESC-3
beobachtet auf dem Rechner, dass für einen angehaltenen Namen vor der Entscheidung keine Abfrage
geschieht: Der Daemon des Laufs fragt über `resolver.nameserver` einen aufzeichnenden Nameserver
(`tests/escape/dns-stub.py`), und `dns_not_before_decision`, `dns_after_allow_once` und
`meta_no_dns_lookup` lesen dessen Log (HUM-115). Der Namensdienst des Systems hängt nach der
Entscheidung an derselben einzigen Aufrufstelle; `daemon/crates/proxy/tests/dns_after_allow.rs`
beweist das im Daemon.

## 8. Das Audit-Log

Die Aufzeichnung ist lokal: Flows, Entscheidungen und Bodies in SQLite unter
`$XDG_DATA_HOME/humanitl/humanitl.db`, große Bodies inhaltsadressiert unter
`$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256>` und zusätzlich das Log
`$XDG_DATA_HOME/humanitl/audit/audit.jsonl` (HUM-050). Jeder Eintrag ist eine Zeile kanonisches
JSON mit `seq`, `ts`, `session`, `kind`, `data`, dem Hash des Vorgängers (`prev`) und seinem
eigenen `hash`, versiegelt mit HMAC-SHA256. Bodies, Header, Klartextwerte von Funden und die Notiz
zu einer Blockierung stehen nie im Log; der Pfad einer Anfrage nur als SHA-256; der Host im
Klartext.

Der HMAC-Schlüssel liegt unter `$XDG_DATA_HOME/humanitl/keys/audit.key` (`0600` in einem
Verzeichnis mit `0700`, mit `AUDIT_005` verweigert, wenn andere ihn lesen können oder er einem
anderen Nutzer gehört). **Verankerung des Kopfes:** alle `audit.anchor_every` Einträge (Vorgabe
100) und wenn der Daemon anhält, steht ein Anker in der Datei **und** derselbe in der Tabelle
`audit_anchors` von `humanitl.db`. Wer nur die Datei ändert, scheitert an der Datenbank; wer
beides ändert, braucht zusätzlich den Schlüssel. Beide Orte liegen auf demselben Rechner. Endet
`audit.jsonl` beim Start vor einem Anker aus `audit_anchors`, startet der Daemon trotzdem, meldet
`AUDIT_007` und setzt die Kette hinter dem letzten Anker mit `audit.resumed` fort; `verify` meldet
die Lücke weiterhin. Passt das Ende selbst nicht (ein letzter Eintrag, der nicht zu Hash oder
Schlüssel passt), hängt der Daemon nichts an (`AUDIT_001`).

### Was die Kette beweist

Sie beweist, dass seit dem Setzen des letzten Ankers kein Eintrag bis zu diesem Anker geändert,
entfernt oder umsortiert wurde, ohne dass `verify` es meldet, solange der Angreifer den
HMAC-Schlüssel nicht hat.

Sie beweist **nicht**:

1. dass der Daemon ehrlich geschrieben hat. Ein manipulierter Daemon protokolliert, was er will;
2. dass keine Ereignisse fehlen, die nie geschrieben wurden. Ein hart beendeter Daemon kann eine
   halbe letzte Zeile schreiben; der nächste Start legt sie beiseite (`AUDIT_002`);
3. dass die letzten bis zu `anchor_every` Einträge nach dem letzten Anker nicht abgeschnitten
   wurden. Ein Schnitt genau an einem Anker hinterlässt eine gültige, kürzere Kette; `verify` warnt
   nur vor unverankerten Einträgen dahinter (`UnanchoredTail`). Ein Daemon, der geordnet anhält,
   verankert seinen letzten Eintrag;
4. irgendetwas gegenüber einem Angreifer, der den Schlüssel hat. Ein Prozess, der als **derselbe
   Nutzer** läuft, hat Schlüssel, Datei und Datenbank und kann Kette und Anker neu aufbauen.

Stärkere Garantien brauchen eine externe Verankerung (nach dem MVP).

**Aufbewahrung: löschen, nie in der Mitte umschreiben (HUM-157, HUM-194).** `audit.retention_days`
und `audit.max_bytes` stehen standardmäßig auf `0`, und dann behält die Kette jeden Eintrag. Ist
einer der beiden gesetzt, löscht ein täglicher Lauf Einträge am **Anfang** der Kette, die ältesten
zuerst, nachdem er die ganze Kette verifiziert hat (**eine Kette, die nicht hält, wird nicht
gekürzt**, `AUDIT_001`), hängt einen versiegelten Eintrag `audit.pruned` an, der Nummer und Hash
des letzten gelöschten Eintrags nennt, gefolgt von einem Anker, und schreibt die Datei neu.
`audit_anchors` wird nie gekürzt. `verify` akzeptiert einen anderen Anfang als 1 nur, wenn ein
`audit.pruned` genau diesen Schnitt nennt; sonst ist es ein `SeqGap`. Was in gelöschten Einträgen
stand, beweist die Kette nicht mehr; ein Export ist die einzige Kopie. Mit einer Byte-Grenze
entscheidet die **Menge** der Einträge, wie lange ein Eintrag bleibt, sodass ein Agent mit vielen
Anfragen frühere Einträge schneller hinausdrängen kann, aber nie unbemerkt.

**Ausfall der Aufzeichnung (HUM-250).** Kann der Recorder seine Datenbank nicht öffnen oder
beschreiben (Platte voll, gesperrt, schreibgeschützt), bekommt jeder Client `RECORDER_001` oder
`RECORDER_003` einmal pro Ursache, ein Client, der sich während des Ausfalls verbindet, bekommt sie
zuerst, und das Fenster markiert jede Karte, die nicht aufgezeichnet wird, und lässt die erste
Freigabe der Session auf eine Bestätigung warten. Der Verkehr wird durch den Ausfall nicht
angehalten; die Aussage „beides wird aufgezeichnet" ist dann sichtbar falsch, nicht still falsch.
`RECORDER_005` beendet den Ausfall.

*Prüfung.* `humanitl audit verify` fragt den Daemon, der mit Schlüssel und Ankern verifiziert
(HUM-156); die Kommandozeile und der Audit-Bildschirm sehen dieselbe Kette mit derselben Stärke
und demselben Kopf-Hash. Ohne Daemon prüft `humanitl audit verify --file` Kette und kanonische Form
**ohne Schlüssel und Anker** und sagt das (`no HMAC key (file mode)`, `no anchors (file mode)`).
`humanitl audit export` prüft nichts; es kopiert oder formatiert, sodass sich auch ein gebrochenes
Log als Beleg übergeben lässt (HUM-214). ESC-5 führt `audit_delete_is_detected` und
`audit_truncate_is_detected` gegen einen echten Daemon aus; Grenze 3 steht als eigener Test
(`truncate_above_last_anchor_not_detected_documented`).

## 9. Was wir in der CI prüfen

Die Garantien in diesem Dokument lassen sich prüfen, nicht nur lesen. Drei Wege, nachzusehen,
vom schnellsten zum gründlichsten:

**1. Die genaue Kommandozeile.** Die vollständige Isolationsrichtlinie ist ein einziger
bwrap-Aufruf. Er wird im Fenster gezeigt (Sandbox, Isolation, „Genauen Befehl anzeigen") und auf
der Kommandozeile:

```sh
humanitl sandbox argv
```

Wer wissen will, was gebunden wird, liest die Argumente `--bind` und `--ro-bind`; wer wissen will,
was nicht gebunden wird, sucht vergeblich nach `/run/user`, `.ssh` und `docker.sock`.

**2. Die Prüfungen.**

```sh
humanitl sandbox check           # three checks, evidence from a running sandbox
humanitl sandbox check --json    # the same, machine-readable
```

Die Prüfungen lesen `/sys/class/net` und `/proc/net/dev`, durchlaufen das Dateisystem **in** der
Sandbox nach Sockets und lesen die Zeile `Seccomp:` aus einem Kindprozess, der den Filter trägt. Schlägt eine Prüfung fehl, wird die Session angehalten,
und ihr Zustand ist `failed`; ein „trotzdem starten" gibt es nicht.

**3. Die Escape-Tests** ([`tests/escape/`](../tests/escape/)). Sie laufen im CI-Job
`escape-tests` auf jedem Issue-Branch und auf `main` und lokal mit `make escape`:

| ID | Datei | Was er zeigt |
|---|---|---|
| ESC-1 | [`esc-1-sockets.sh`](../tests/escape/esc-1-sockets.sh) | Socket-Familien und -Typen, `io_uring`, x32, Interfaces, Routing, Capabilities, `Seccomp: 2` für jeden Prozess, PID 1, gemeldete Verweigerungen |
| ESC-2 | [`esc-2-mounts.sh`](../tests/escape/esc-2-mounts.sh) | Mount-Fläche, genau ein Socket, kein `/proc` des Rechners, Hostname |
| ESC-3 | [`esc-3-egress.sh`](../tests/escape/esc-3-egress.sh) | kein Egress ohne den Proxy; durch den Proxy landet jede Anfrage, über die keine Regel entscheidet, in der Warteschlange (`allow`-Regeln und die LLM-Durchleitung überspringen sie); kein DNS vor der Entscheidung, auf dem Host am Log eines aufzeichnenden Nameservers bewiesen |
| ESC-4 | [`esc-4-rules.sh`](../tests/escape/esc-4-rules.sh) | die Regeltabelle einschließlich Homograph, IP-Literal, WebSocket und Body-Grenze |
| ESC-5 | [`esc-5-filesystem.sh`](../tests/escape/esc-5-filesystem.sh) | Markierung von Symlinks, verdeckte Pfade, OSC 52, Manipulation des Audits, ein Wegwerf-Lauf, der nie ins Projekt schreibt (HUM-350) |

Neben den Escape-Tests führt die CI die Konformitätsmatrix echter Clients durch den Proxy aus
(`daemon/crates/proxy/tests/conformance.rs`), den Lasttest für Gegendruck (1000 Flows in 10 s),
die Prüfung, dass der Shim statisch und kleiner als 2 MB ist, und das nächtliche Fuzzing.

## 10. Eine Schwachstelle melden

Bitte melde ein Sicherheitsproblem **nicht** als öffentliches Issue.

Kontakt: siehe GitHub Security Advisories für dieses Repository (private Meldung von
Schwachstellen).

Meldeformular: <https://github.com/nurkert/Humanitl/security/advisories/new>

- **Was hilft:** die betroffene Version, das Sandbox-Profil (`humanitl sandbox argv`), eine
  minimale Reproduktion und, wenn möglich, die Beobachtung als Probe für einen Escape-Test im Stil
  von `tests/escape/`.
- **Antwort:** Eingangsbestätigung innerhalb von 72 Stunden, Bewertung innerhalb von sieben Tagen.
- **Veröffentlichung:** koordinierte Veröffentlichung nach 90 Tagen oder sobald eine Behebung
  verfügbar ist, je nachdem, was zuerst eintritt. Willst du früher veröffentlichen, sag das bitte
  in der Meldung.
- **Keine Bug-Bounty.** Das ist ein Projekt mit einem einzigen Maintainer unter GPL-3.0. Eine
  Nennung im Änderungsprotokoll unten gibt es gern, mit Namen; Geld nicht.

## 11. Änderungsprotokoll sicherheitsrelevanter Änderungen

Jede sicherheitsrelevante Änderung am Shim, an der Erlaubnisliste der Mounts, am seccomp-Filter, an
der Durchleitung oder an der Audit-Kette bekommt hier eine Zeile und in der Änderungshistorie von
[`THREAT-MODEL.md`](THREAT-MODEL.md#change-history).

| Version | Änderung |
|---|---|
| 0.1.0 | Erste Veröffentlichung. |
| 0.1.0 | Mehrere gespeicherte Modellserver mit je einem Eintrag im Schlüsselbund; nur der aktive Origin bekommt einen Schlüssel und die Durchleitung (HUM-289). |
| 0.1.0 | Der erste Prozess der Session nimmt das Pseudo-Terminal des Daemons als steuerndes Terminal; `--new-session` bleibt. Der Befehl eines Aufrufers bekommt den Beitrag des Adapters und den eingerichteten Agenten im `PATH`, nie einen eigenen Bind (HUM-280). |
| 0.1.0 | OpenCode fragt nicht mehr vor `webfetch` und `bash`; der Proxy ist die eine Stelle, die fragt. Sein `bash` wartet so lange wie eine angehaltene Anfrage (HUM-308). |
| 0.1.x | Weitere Shells neben dem Agenten: Der Daemon bittet PID 1 über ein unbenanntes `SOCK_SEQPACKET`-Paar, das nur er und PID 1 halten; der Shim forkt die Shell von innen mit dem Filter des Agenten darüber, bewiesen durch ein verweigertes `AF_UNIX`; höchstens vier, sie enden mit ihrem Prozess oder der Sandbox (HUM-299). |
| 0.1.x | pi lädt die global installierte Erweiterung `pi-web-access` (`agent.pi.web_access`, Vorgabe `auto`, für Projekt-Profile gesperrt); ihre Anfragen gehen wie jede andere durch den Proxy (HUM-309). |
| 0.1.x | macOS: Seatbelt als Sandbox-Backend, ein erzeugtes SBPL-Profil pro Session, die drei Garantien neu formuliert und vom Shim gemessen; der Daemon als launchd-Agent; Socket und Token unter `~/Library/Application Support/Humanitl/run` (HUM-323, HUM-327, [Abschnitt 12](#12-macos-seatbelt-instead-of-bubblewrap)). |

## 12. macOS: Seatbelt statt bubblewrap

macOS hat keinen Netzwerk-Namespace, keinen Mount-Namespace und kein seccomp. Dort ist die Sandbox
Seatbelt, die Sandbox des macOS-Kernels: Der Daemon schreibt pro Session ein SBPL-Profil und
startet den Shim unter `/usr/bin/sandbox-exec -f <profile>`. Der Kernel hält den Shim und jeden
Prozess, den er startet, an dieses Profil; kein Kindprozess kann es ablegen. Die Entscheidung und
die Alternativen, darunter eine Linux-VM mit dem bwrap-Backend darin (HUM-328), stehen in
[ADR-0019](adr/0019-seatbelt-on-macos.md).

### Die drei Garantien unter macOS

| # | Unter Linux | Unter macOS | Vom Shim gemessen als |
|---|---|---|---|
| 1 | Kein Netzwerk-Interface | **Kein Weg nach draußen.** Die Interfaces existieren, aber jedes `connect`, `bind` und `sendto` außer den dreien unten endet mit `EPERM`; es gibt keine Namensauflösung. | eine TCP-Verbindung zu einer öffentlichen IPv4- und IPv6-Adresse und ein UDP-Datagramm, alle verweigert (`no_interfaces`) |
| 2 | Genau eine Tür | **Genau eine Tür.** Erlaubt sind der Proxy-Socket des Daemons, der Loopback-Port der Session, über den der Shim zu ihm weiterleitet, und ein zweiter, zufälliger Port der Session für den eigenen lokalen Server des Agenten (HUM-365). | die Tür antwortet, `/var/run/syslog` und ein fremder Loopback-Port nicht (`single_socket`, `bridge_listening`) |
| 3 | seccomp | **Der Kernel hält den Prozess an das Profil.** Routing-, System- und Treiber-Sockets werden bei `socket(2)` verweigert; unprivilegiertes ICMP hat keine entfernte Adresse, die das Profil erlaubt. | `sandbox_check(getpid())` meldet den Prozess als in der Sandbox; `AF_ROUTE`, `AF_SYSTEM`, `AF_NDRV` und `AF_INET SOCK_RAW` enden mit `EPERM` (`seccomp_applied`, `families`). Der Raw-Socket wird jedem unprivilegierten Prozess ohnehin verweigert; er bleibt als Boden in der Liste. |

Die Namen der Prüfungen sind die von Linux, sodass die Isolationsanzeige und
`humanitl sandbox check` denselben Bericht lesen; die Spalte mit den Belegen sagt, was auf welchem
System gemessen wurde.

### Das Profil

Das Profil wird von `SeatbeltPolicy::render` in `daemon/crates/sandbox/src/seatbelt.rs` erzeugt und
liegt, für den Agenten schreibgeschützt, als `sandbox.sb` im Session-Verzeichnis. Es verweigert
standardmäßig. In SBPL gewinnt die letzte passende Regel, deshalb kommen die Verweigerungen der
Masken zuletzt.

- **Lesen:** das ganze System außer `/Users`, `/Volumes`, `/private/var/folders` und
  `/private/tmp`; von diesen nur das Projekt, das Session-Verzeichnis, die zusätzlichen Ordner des
  Profils und einzelne Dateien: der Shim und das Programm des Agenten, das der Start aufgelöst hat.
  Die Ordner darüber dürfen nach Namen aufgelistet (`file-read-metadata`), nicht gelesen werden.
- **Schreiben:** das Projekt (wenn es `rw` eingehängt ist), das eigene `home` und `tmp` der
  Session, die zusätzlichen beschreibbaren Ordner, `/dev/null` und das Terminal der Session. Nie
  das Session-Verzeichnis selbst: Dort liegt das Profil weiterer Shells.
- **Netzwerk:** ausgehend nur zum Proxy-Socket und zu `localhost:<port>` der Session; Binden und
  eingehend nur auf diesem Port.
- **Masken:** Die `masked_files` und die `tmpfs`-Pfade unter `/work`, die im Projekt existieren,
  sind weder lesbar noch beschreibbar, und die Ordner zwischen dem Projekt und einer Maske dürfen
  nicht umbenannt oder entfernt werden, sodass `mv .git .gitx` die Maske von `.git/hooks` nicht
  aufheben kann. Eine Maske, deren Pfad nicht existiert, wird wie unter Linux als ungeschützt
  gemeldet.
- **Terminals:** Andere Terminals des Nutzers (`/dev/ttys*`) können weder gelesen noch beschrieben
  werden. Der Agent behält das Terminal, das er geerbt hat, erreicht es als `/dev/tty` und darf
  neue Pseudo-Terminals anlegen; deren Follower-Seite trägt die Erweiterung
  `com.apple.sandbox.pty`, die nur sie bekommt.
- **Dienste:** nur die Mach-Dienste, die gewöhnliche Werkzeuge für Nutzerdaten, Benachrichtigungen,
  Logging und Einstellungen brauchen. Nicht erreichbar: `mDNSResponder` (DNS), `trustd`,
  `securityd` (also kein Keychain), die Zwischenablage (Pasteboard) und launchd.

Ohne Mounts werden die Sandbox-Pfade eines Profils abgebildet: `/work` ist das Projekt selbst,
`/home/agent`, `/tmp`, `/etc/humanitl` und `/run/humanitl` liegen im Session-Verzeichnis unter dem
Laufzeitverzeichnis. Die Abbildung gilt für die Umgebung, den Befehl und den Text der
Adapter-Dateien, sodass der Agent seine Konfiguration dort findet, wo der Adapter sie abgelegt hat.

### Was anders ist, und was daraus folgt

Das sind die Punkte, an denen macOS weniger bietet als Linux. Jeder ist ein Kanal in
[`THREAT-MODEL.md`](THREAT-MODEL.md) mit seiner Folge.

0. **Die Dateien einer Session liegen auf der Platte** (HUM-368). Unter Linux sind sie memfds. Unter
   macOS liegen sie im Session-Verzeichnis, `0700` unter `~/Library/Application Support/Humanitl/run/sessions`,
   auch der Zugangsschlüssel des Modells, und das Verzeichnis verschwindet mit dem Ende der Sandbox.
   Nach einem Absturz des Daemons bleibt es, bis ein späterer Plan es wegräumt, höchstens sieben Tage,
   und ein Backup während einer Session kann es enthalten.
0. **Ein Projekt kann OpenCode nicht konfigurieren** (HUM-371). Das verwaltete
   Konfigurationsverzeichnis von OpenCode liegt unter macOS außerhalb jeder Session, also schaltet der
   Adapter die Konfiguration des Projekts mit `OPENCODE_DISABLE_PROJECT_CONFIG` ab. Eine
   `opencode.json` im Projekt wirkt dort in der Sandbox nicht, ebenso wenig sein
   `.opencode`-Verzeichnis oder seine `AGENTS.md`; unter Linux zählen sie, unter der verwalteten Datei.
1. **Loopback wird mit dem Rechner geteilt** (K-19). Die Tür lauscht auf einem Loopback-Port, den
   jeder lokale Prozess jedes Kontos erreichen kann. Ein solcher Prozess erreicht nur den Proxy,
   aber mit den Regeln der Session: Was eine `allow`-Regel oder die LLM-Durchleitung durchlässt,
   geht ohne Anhalten hinaus, alles andere landet in deiner Warteschlange. Der Agent wiederum
   erreicht keinen anderen Loopback-Port als die Tür und den einen zufälligen Port der Session für
   seinen eigenen Server (HUM-365), kann also nicht mit Datenbanken oder anderen Diensten
   des Rechners sprechen.
2. **Es gibt keinen PID-Namespace** (K-20). Der Befehl läuft in einer eigenen Session; an ihrem
   Ende und drei Sekunden nach einem Anhalten beendet der Shim die Prozesse dieser Session, in
   Durchgängen, die sich wiederholen, bis einer niemanden mehr findet (höchstens zehn). Das ist
   nach bestem Bemühen, kein Namespace: Ein Prozess, der eine weitere Session startet, überlebt
   die Sandbox, und ebenso einer, der schneller forkt, als zehn Durchgänge brauchen. Ein solcher
   Prozess bleibt unter dem Profil, das ihm den Proxy-Socket des Daemons direkt erlaubt: Solange
   der Daemon läuft, kann er weiter Anfragen schicken, die Durchleitung und `allow`-Regeln gelten,
   und alles andere wird nach dem Ende der Session in der Warteschlange angehalten.
   Unter Linux beendet der PID-Namespace jeden Prozess, und seccomp verweigert `AF_UNIX`. Den
   Proxy-Socket auf den Shim der laufenden Session zu beschränken, ist HUM-331. Bis dahin starte
   nach einer Session unter macOS, der du nicht traust, den Daemon neu (`humanitl daemon restart`).
3. **Eine Sperrliste statt einer Erlaubnisliste fürs Lesen** (K-21). Unter Linux sieht die Sandbox
   `/usr`, `/etc/ssl` und ein paar Dateien, mit einer erzeugten `passwd` und `hosts`. Unter macOS
   liest sie alles außerhalb von `/Users`, `/Volumes`, `/private/var/folders` und `/private/tmp`,
   darunter ganz `/etc`, `/Library`, `/Applications`, `/opt/homebrew` und `/usr/local/etc`. Eine
   Zugangsangabe in einer Konfigurationsdatei dort (einer Datenbank oder eines Homebrew-Dienstes)
   kann der Agent lesen, und er könnte sie ohne Anhalten an die LLM-Durchleitung schicken. Bewahre
   Secrets in deinem Home-Verzeichnis auf. Das Profil nennt außerdem Pfade: Das
   Projektverzeichnis wird vor dem Start aufgelöst und geprüft, aber nicht wie unter Linux über
   einen Deskriptor festgehalten (HUM-263).
4. **Seatbelt ist veraltet und nur durch Beispiele dokumentiert** (K-22). Die Escape-Suite für
   macOS läuft im CI-Job `macos` bei jedem Commit und gegen die Programme in jedem Release, auf dem
   macOS der `macos-15`-Runner von GitHub; während der Entwicklung lief sie außerdem unter
   macOS 27. Andere Versionen sind nicht getestet. Auf jeder Version misst der Shim die drei
   Garantien bei jedem Start, und eine rote Zeile hält die Session wie unter Linux an (K-15),
   sodass ein macOS, das nicht mehr verweigert, was das Profil verbietet, eine rote
   Isolationsprüfung zeigt statt eines laufenden Agenten.
5. **Seitenkanäle innerhalb des Rechners, die keinen neuen Weg nach draußen tragen** (K-23):
   POSIX-Semaphoren und Shared Memory, das Systemlog. Keiner davon verlässt den Rechner.

### Was wir für macOS prüfen

`tests/escape/macos/run.sh` startet einen Daemon in einem leeren Home und führt
`humanitl sandbox check` aus, dessen Shim-Zeilen `EPERM` für die in der Tabelle oben aufgeführten
Proben zusichern. Dann versucht es aus einer echten Sandbox heraus jeden Weg nach draußen, den das
Profil verweigern muss, und zählt eine Probe als verweigert, wenn der Befehl fehlschlägt (die
Zeilen des Shims sind die, die die errno nennen): direktes TCP und TLS,
DNS, UDP, IPv6, Raw- und Routing-Sockets, `syslog` und `mDNSResponder`, einen anderen
Loopback-Port, das Lesen des Home-Verzeichnisses, von `/Users` und der verdeckten `.env`, das
Schreiben von `.git/hooks`, `/private/tmp` und `/Library`, das Umschreiben oder Entfernen des
Profils, das Schreiben der Adapter-Dateien, das Umbenennen von `.git`, das Öffnen eines anderen
Terminals. Es prüft, dass ein Hintergrundprozess mit der Sandbox endet, dass der Agent weiterhin
ein Pseudo-Terminal anlegen kann und dass eine Anfrage durch die Tür als Flow beim Daemon ankommt
(dass der Proxy sie anhält, ist Gegenstand der Linux-Suite, ESC-4). Es läuft im CI-Job `macos` und im Release-Job `check-macos` gegen
die Binaries im Disk-Image.

| 0.1.0 | Die Wegwerf-Sandbox: der Projektordner als schreibgeschützte untere Schicht eines Overlays, die obere Schicht im Arbeitsspeicher unter dem Laufzeitverzeichnis mit einer Größengrenze, `mounts.extra_rw` schreibgeschützt; Änderungen werden nur bei der Übernahme zurückgeschrieben, nie durch einen Symlink, nie als Symlink aus dem Projekt hinaus, als Gerät oder mit setuid-Bits und nie über eine Datei, die der Mensch seit dem Start geändert hat, ohne `--overwrite` (HUM-350, K-24). |
| 0.1.0 | Der Bereich für Änderungen in der App: Ein Konflikt wird nur überschrieben, wenn er für genau diese Änderung bestätigt wurde (`ApplyChanges.overwrite_ids`); ein Konflikt, der nach der Auswahl auftritt, stoppt die Übernahme (HUM-351, K-24). |
