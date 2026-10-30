---
source: adr/0019-seatbelt-on-macos.md
source-sha256: c263471f377309c52eff07ed553441ccbd1bc7bf7b4b9c1bef3e45eff79782f3
lang: de
---
# ADR-0019 · Seatbelt ist die Sandbox auf macOS, eine Linux-VM folgt später

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-10-30

## Kontext

Humanitl soll auf macOS so sauber laufen wie auf Linux, und jedes Release soll eine Mac-App
mitbringen (Wunsch des Eigentümers vom 2026-10-29). Die Sandbox aus ADR-0002 beruht auf
Funktionen des Linux-Kernels, die macOS nicht hat: einem leeren Netzwerk-Namespace,
Mount-Namespaces, seccomp. Ohne sie lassen sich die drei Garantien aus
[`SECURITY.md`](../SECURITY.md) nicht so aussprechen, wie sie dastehen.

Der Port `SandboxBackend` (ADR-0015) nennt Seatbelt und eine Micro-VM als Kandidaten für ein
zweites Backend. Auf macOS standen drei Wege offen:

- **Seatbelt.** Die Sandbox des macOS-Kernels: `sandbox-exec -f profile.sb` setzt ein
  SBPL-Profil, das der Kernel für den Prozess und jedes Kind durchsetzt. Chrome, OpenAI Codex
  und Claude Code `sandbox-runtime` nutzen es. Es braucht keine Rechte und keinen Download.
- **Eine Linux-VM** über `Virtualization.framework`, mit dem bwrap-Backend darin und der Tür
  als vsock-Port. Dieselben drei Garantien wie auf Linux, zum Preis eines Gast-Images im
  Release, eines Bootvorgangs je Session und eines zweiten Kernels, den man aktuell halten
  muss.
- **Docker Desktop oder ein ähnliches VM-Werkzeug.** Eine Abhängigkeit, die der Nutzer
  getrennt installiert und lizenziert, und ein Daemon, der als privilegierter Helfer läuft.

## Entscheidung

Auf macOS ist das Sandbox-Backend Seatbelt (`SeatbeltBackend` in
`daemon/crates/sandbox/src/seatbelt.rs`), zur Übersetzungszeit über den Typalias
`PlatformBackend` gewählt. Daemon und Kommandozeile nennen nur diesen Alias.

- Der Plan schreibt je Session ein SBPL-Profil in ein privates Session-Verzeichnis und startet
  `/usr/bin/sandbox-exec -f <profile> <shim> …`. Das Profil verbietet standardmäßig alles;
  was es erlaubt, steht in
  [`SECURITY.md` Abschnitt 12](../SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap).
- Ohne Mounts werden die Sandbox-Pfade des Profils (`/work`, `/home/agent`, `/tmp`,
  `/etc/humanitl`, `/run/humanitl`) auf das Projekt und das Session-Verzeichnis abgebildet,
  in der Umgebung, im Befehl und in den Adapter-Dateien.
- Der Shim läuft auch auf macOS, ohne seccomp. Er bedient die Tür auf einem freien
  Loopback-Port der Session, misst die Garantien von innen und schreibt dieselben fünf
  `CHECK`-Zeilen; der Host bildet sie wie auf Linux auf die drei Garantien ab.
- Die Garantien werden für macOS neu formuliert: kein Weg nach draußen (jede Verbindung außer
  der Tür endet in `EPERM`), genau eine Tür, und der Kernel hält den Prozess an das Profil
  (`sandbox_check`).

Ein Backend mit einer Linux-VM folgt als eigenes Issue (HUM-328), vom Eigentümer am
2026-10-29 entschieden.

## Begründung

Seatbelt trägt den Teil des Anspruchs, auf den es bei der Ausleitung ankommt: Der Shim sichert
für ein öffentliches TCP- und UDP-Ziel, IPv6, fremde Unix- und Loopback-Ziele
sowie Raw-, Routing- und System-Sockets `EPERM` zu, und die macOS-Escape-Suite
(`tests/escape/macos/run.sh`) prüft aus einer echten Sandbox heraus, dass direktes TCP, TLS,
DNS und diese Sockets scheitern, dass Home-Verzeichnis, Masken, andere Terminals und das
Profil selbst außer Reichweite sind und dass eine Anfrage durch die Tür beim Daemon ankommt.
Eine Mutation, die das Netzwerk öffnet, färbt die Garantien 1 und 2 rot. Die Suite läuft im
CI-Job `macos` und in jedem Release gegen die gebündelten Programme.

Sie kostet nichts bei der Installation und startet in Millisekunden, also bekommt ein
Mac-Nutzer denselben Arbeitsablauf wie ein Linux-Nutzer.

## Verworfene Alternativen

- **Die Linux-VM jetzt.** Sie würde die Linux-Garantien Wort für Wort erhalten, braucht aber
  vor dem ersten Mac-Release ein Gast-Image, dessen Updates und eine vsock-Tür; der Eigentümer
  hat sie hinter Seatbelt gestellt.
- **Docker Desktop.** Eine getrennt lizenzierte Abhängigkeit mit privilegiertem Helfer, und
  `docker.sock` ist ein weiterer Weg hinein (ADR-0002).
- **Keine Sandbox auf macOS.** Der Proxy allein würde darauf beruhen, dass der Agent
  `HTTP_PROXY` beachtet; das ist Vertrauen statt Physik (Grundsatz 1).

## Konsequenzen

- Garantie 1 ändert auf macOS ihren Wortlaut: Interfaces gibt es, aber kein Prozess in der
  Sandbox darf sie benutzen. Garantie 3 ist das Profil, nicht seccomp.
- Loopback teilt sich die Sandbox mit dem Host, also lauscht die Tür auf einem Port, den der
  Host sieht. Jeder lokale Prozess jedes Kontos kann sie erreichen und sendet mit den Regeln
  der Session (K-19).
- Es gibt keinen PID-Namespace. Der Shim beendet die Session des Befehls am Ende der Sandbox,
  in wiederholten Durchgängen; ein Prozess, der eine eigene Session startet, überlebt sie,
  weiterhin unter dem Profil, und das Profil erlaubt ihm den Proxy-Socket des Daemons, solange
  der Daemon läuft (K-20). HUM-331 beschränkt diesen Socket auf den Shim der Session.
- Lesen regelt eine Verbotsliste, nicht die Erlaubnisliste der Mounts: Alles außerhalb von
  `/Users`, `/Volumes` und den Temp-Bäumen ist lesbar, `/etc` und `/opt/homebrew`
  eingeschlossen. Das Projekt ist nicht wie auf Linux per Deskriptor festgelegt (K-21).
- Apple kennzeichnet `sandbox-exec` als veraltet und dokumentiert SBPL nur in Beispielen.
  Jedes Release lässt die Escape-Suite gegen das macOS des Runners `macos-15` laufen, also
  zeigt sich eine Änderung in dieser Version als roter Release-Lauf; eine, die erst ein neueres
  macOS bringt, zeigt sich, wenn die Runner umziehen (K-22).

## Betroffene Issues

`HUM-323` (das Backend), `HUM-327` (diese Entscheidung und die Sicherheitsdokumente),
`HUM-328` (das VM-Backend). Siehe [`backlog/sprint-6.md`](../../backlog/sprint-6.md).
