---
source: adr/0002-bwrap-first.md
source-sha256: 39f6937ab42ae697e347d786f362d2b2fdcf5f6130113a659dcb852da709febc
lang: de
---
# ADR-0002 · Zuerst bubblewrap, Docker später als zweites Backend

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Der Agent muss in einer Umgebung laufen, in der es physisch keinen Weg ins Netz gibt
außer dem einen Socket, den Humanitl kontrolliert. Das ist Prinzip 1: Physik statt
Vertrauen. Ein Agent, der `HTTP_PROXY` ignoriert oder seine eigene DNS-Auflösung
mitbringt, darf nicht durchkommen; er muss scheitern (fail-closed).

Daraus folgen drei Garantien, und sie sollen live in der Oberfläche überprüfbar sein
(`BACKLOG.md` 4.1):

1. **Kein Netzwerk-Interface.** In der Sandbox gibt es nur `lo`. Keine IP, kein DNS,
   kein ICMP, kein QUIC, keine Raw-Sockets.
2. **Genau eine Tür.** Der Proxy-Socket des Daemons wird als einzelne Datei in die
   Sandbox eingebunden; `find / -type s` liefert genau diese eine Datei.
3. **Keine neuen Türen.** Ein seccomp-Filter verhindert, dass in der Sandbox überhaupt
   andere Socket-Familien oder -Typen geöffnet werden.

Die Sandbox-Technik muss diese drei Garantien tragen, ohne root funktionieren (die
Zielgruppe installiert ein `.deb` und will kein `sudo` tippen), ohne Hintergrunddienst
auskommen und ihre Richtlinie in einer Form ausdrücken, die die Oberfläche einem
Nicht-Fachmann zeigen kann.

## Entscheidung

Das Sandbox-Backend im MVP ist
[bubblewrap](https://github.com/containers/bubblewrap) (`bwrap`), angesprochen über den
Port `SandboxBackend` in `daemon/crates/sandbox`. Die Richtlinie ist eine einzige
lesbare `bwrap`-Kommandozeile, erzeugt aus einem Profil unter `profiles/sandbox/*.toml`
und ungekürzt in der Oberfläche angezeigt („Genaue bwrap-Kommandozeile anzeigen“).

Kern der Kommandozeile: `--unshare-all --new-session --die-with-parent
--cap-drop ALL`, dazu `--unshare-pid --unshare-ipc --unshare-uts`, ein eigenes
`/proc`, `--tmpfs /dev/shm`, der Hostname `sandbox` und eine Allowlist für Mounts.
`--cap-drop ALL` steht bedingungslos im argv, direkt nach `--new-session`, und ist
nicht konfigurierbar (`backlog/CONVENTIONS.md` 4.11). Nie eingehängt werden
`$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `~/.ssh`, `~/.gitconfig`, `~/.netrc` sowie
X11-, Wayland-, dbus- und docker-Sockets. `/work` ist der Projektordner, entweder `ro`
oder `rw`; `.git/hooks`, `.git/config`, `.envrc`, `.vscode` und `.idea` sind mit tmpfs
maskiert.

Docker kommt nach dem MVP als zweite Implementierung desselben Traits
(`SandboxBackend`), für vollständige Toolchain-Images. Kern und Anwendung bleiben davon
unberührt (ADR-0015).

## Begründung

`--unshare-all` gibt der Sandbox einen leeren Netzwerk-Namespace: Es gibt nur `lo`, die
Routing-Tabelle ist leer. Garantie 1 ist damit keine Frage der Konfiguration, sondern
eine Eigenschaft des Kernels. Wichtig ist die Nebenwirkung auf abstrakte Unix-Sockets:
Der abstrakte Namensraum ist an den Netzwerk-Namespace gebunden, also ist er in der
Sandbox ebenfalls leer. Ein Agent kann sich deshalb nicht an einen abstrakten Socket
des Hosts hängen.

bwrap läuft ohne root über User-Namespaces und braucht keinen Daemon. Die Richtlinie ist
ein `argv`, kein Zustandsobjekt in einem fremden Prozess: Sie lässt sich anzeigen,
kopieren, an einen Fehlerbericht hängen und außerhalb von Humanitl nachstellen. Für ein
Werkzeug, dessen Sicherheitsversprechen der Nutzer nachvollziehen können soll, ist diese
Durchsichtigkeit das entscheidende Argument.

bwrap ist in Debian und Ubuntu paketiert und wird von Flatpak in großem Umfang genutzt,
es ist also weder exotisch noch unbeobachtet.

Die Grenzen werden nicht verschwiegen: bwrap braucht unprivilegierte User-Namespaces.
Ab Ubuntu 24.04 schränkt ein AppArmor-Profil sie ein; `humanitl doctor` (HUM-075) prüft
`unprivileged_userns_clone` und das AppArmor-Profil und liefert bei einem Fehlschlag
eine konkrete Abhilfe statt einer Fehlermeldung.

## Verworfene Alternativen

- **Docker mit `--network none` (im MVP).** Legt ebenfalls einen leeren
  Netzwerk-Namespace an, braucht aber trotzdem eine Bridge und einen seccomp-Filter im
  Container, läuft über einen Daemon als root, und `docker.sock` ist ein zusätzlicher
  Weg für Fallstricke: Wer ihn erreicht, ist root auf dem Host. Für die Zielgruppe ist
  „Docker installieren“ außerdem eine Hürde vor dem ersten Start. Kommt nach dem MVP als
  zweites Backend, wo Toolchain-Images den Aufwand rechtfertigen.
- **gVisor.** Ohne root und ohne eigenen Netstack für unser Modell unbrauchbar, und die
  Kompatibilitätsfläche für beliebige Agenten-Toolchains ist schwer abzuschätzen.
- **nsjail.** Technisch nah an bwrap, aber kein Debian-Paket. Das Lieferproblem („ein
  Paket, alles drin“) wiegt schwerer als der Unterschied im Funktionsumfang.
- **Firecracker oder Kata Containers.** Stärkere Isolation über KVM, aber schwer an
  Werkzeugen, Kernel-Image im Paket, langsamer Start, KVM-Zugriff nicht überall
  verfügbar. Vorgemerkt als v2 für ein stärkeres Bedrohungsmodell, nicht für den MVP.
- **Nur seccomp, ohne Namespaces.** Ein seccomp-Filter allein hält kein Dateisystem und
  keine Prozessliste zurück; in unserem Entwurf ist er das Sicherheitsnetz
  (Garantie 3), nicht die tragende Wand.

## Konsequenzen

- Die Sandbox ist Linux-spezifisch. macOS bräuchte ein eigenes Backend (`seatbelt`), das
  der Port `SandboxBackend` zulässt, das aber niemand baut.
- Ein Agent in der Sandbox erreicht den Proxy über `127.0.0.1:3128`, weil es im leeren
  Netzwerk-Namespace nur Loopback gibt. Die Übersetzung von diesem TCP-Endpunkt auf den
  Unix-Socket des Hosts übernimmt der Shim (`humanitl-shim`). Das Prozessmodell kam
  ursprünglich ohne `--as-pid-1` aus: PID 1 in der Sandbox war das init von bwrap, der
  Shim lief darunter. Seit HUM-203 gilt der Nachtrag unten: bwrap startet mit
  `--as-pid-1`, der Shim ist PID 1. Die Isolationsprüfung liest weiterhin das gefilterte
  Kind. Einzelheiten zur Liste der Bridges in ADR-0016.
- Weil es im Namespace nur `lo` gibt, darf seccomp `AF_INET`/`AF_INET6` mit
  `SOCK_STREAM` erlauben, ohne die Garantie zu schwächen. Der Filter verhält sich genau
  so (`backlog/CONVENTIONS.md` 4.10 und 4.11): `socket()` ist nur für das Kreuzprodukt
  `allow_families` × `allow_types` aus dem Profil erlaubt; `arg1` wird mit `0xff`
  maskiert, damit `SOCK_NONBLOCK|SOCK_CLOEXEC` durchgehen. Jede andere Familie und jeder
  andere Typ liefert `EPERM`. Eine abweichende Architektur führt zu `KillProcess`.
  x32-Syscalls (`nr & 0x40000000`) liefern `EPERM` über einen handgeschriebenen
  BPF-Vorspann vor dem seccompiler-Programm; das ist Teil der Entscheidung, nicht „bei
  Bedarf“. `socketpair()` bleibt vom Filter unberührt: Es kennt nur `AF_UNIX`, verbindet
  zwei Deskriptoren desselben Prozessbaums und ist kein Egress (Node und Bun brauchen es
  für die IPC ihrer Kindprozesse). Der Filter steht in
  `daemon/bin/humanitl-shim/src/seccomp.rs` und führt unter `#[cfg(test)]` eine Tabelle
  aller Regeln; `docs/SECURITY.md` zitiert die Liste von dort. Das Profil `browser`
  erweitert die Familien später um `AF_UNIX` (ADR-0016); die Garantie tragen dann
  weiterhin der Netzwerk-Namespace und die Allowlist für Mounts.
- Die bwrap-Version und die Verfügbarkeit von User-Namespaces werden zur Voraussetzung
  der Installation. `humanitl doctor` prüft beides und bietet den Paketbefehl zum
  Kopieren an; bei einem Fehlschlag sperrt der Bildschirm „Sandbox“ den Start mit einem
  konkreten Grund und bietet nie „trotzdem starten“ an.
- Die drei Garantien sind testbar und werden getestet: Escape-Test 1
  (Socket-Verweigerung, erwartet `socketpair()` = ok) und 2 (Mount-Fläche) prüfen sie in
  der laufenden Sandbox, der Bereich „Isolation“ zeigt dieselben Prüfungen live in der
  Oberfläche.

## Nachtrag 2026-09-23: Der Shim ist PID 1 (HUM-203)

Der Sicherheitsdurchgang vom 2026-09-23 (Befund M1) zeigte, dass das Prozessmodell ohne
`--as-pid-1` eine Lücke hatte. Das init von bwrap trug keinen seccomp-Filter, war
„dumpable“ und lief unter derselben UID im selben User-Namespace wie der Agent. Mit
`kernel.yama.ptrace_scope = 0` öffnete der Agent `/proc/1/mem` mit `O_RDWR` (gemessen
gegen bwrap 0.13.0) und hätte Code ohne Filter ausführen können. `/proc/<pid>/mem` ist
kein Syscall, und `pidfd_getfd` stand nicht in der Grundsperre; beide Wege prüft nur
`ptrace_may_access`.

Entscheidung: `to_bwrap_args` setzt `--as-pid-1` in jede Kommandozeile, ohne Schalter.
Das Flag verlangt `--unshare-pid`, das jedes Profil ohnehin setzen muss, und verträgt
sich nicht mit `--lock-file`, das Humanitl nicht nutzt. `--as-pid-1` allein reicht nicht,
weil dann der Elternprozess des Shims das beschreibbare PID 1 wäre. Deshalb setzt der
Elternprozess als ersten Schritt nach dem `fork` `PR_SET_DUMPABLE` auf 0 und öffnet das
Tor zum Agenten erst, wenn `PR_GET_DUMPABLE` es bestätigt (schon so seit HUM-138, jetzt
tragend für PID 1). `pidfd_getfd` kommt in die Grundsperre, `pidfd_open` nur in die Liste der
benennbaren Syscalls, weil asyncio, Go und libuv danach tasten.

Konsequenzen: Als PID 1 erntet der Shim jede Waise (`waitpid(-1)`) und zählt nur den
Status seines eigenen Kindes. Das Weiterreichen von `SIGTERM` und `SIGHUP` ist tragend,
weil der Kernel einem Namespace-init keine Signale ohne Handler zustellt. Endet der Shim,
endet die Sandbox. `/proc/1/status` zeigt jetzt `Seccomp: 2`, und ESC-1 prüft PID 1 ohne
Ausnahme.

## Betroffene Issues

`HUM-010` (Format der Sandbox-Profile), `HUM-011` (bwrap-Launcher), `HUM-012`
(humanitl-shim mit seccomp), `HUM-013` (Einbinden des Proxy-Sockets), `HUM-006`
(Harness der Escape-Tests), `HUM-040` (Bildschirm „Sandbox“), `HUM-041`
(Bereich der Isolationsprüfung), `HUM-043` (Härtung von `/work`), `HUM-075`
(`humanitl doctor`), `HUM-203` (`--as-pid-1`, Nachtrag).
