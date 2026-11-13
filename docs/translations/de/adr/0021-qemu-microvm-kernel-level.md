---
source: adr/0021-qemu-microvm-kernel-level.md
source-sha256: 853a6a5421dc4667ba7e9054c2f2c732620c84205e36c07d218c0d90bd75fdce
lang: de
---
# ADR-0021 · Die Kernel-Stufe läuft als QEMU-Micro-VM aus Debian-Paketen

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-13

## Kontext

ADR-0020 hat die Kernel-Stufe als Podman mit der OCI-Laufzeit `krun` geplant (crun, gebaut mit
libkrun). Weder libkrun noch ein crun mit libkrun-Unterstützung ist in Debian paketiert; nur die
Nutzlast für den Gast, `libkrunfw`, ist es. Die Stufe hätte bedeutet, auf jedem Rechner, der sie
will, fremden Code herunterzuladen und zu bauen und diesen Build selbst aktuell zu halten. Der
Eigentümer hat sich am 2026-11-13 gegen diesen Weg entschieden: Die Kernel-Stufe nutzt nur
Software, die die Distribution ausliefert, und Podman bleibt, was es heute ist, die Engine der
gemeinsamen Stufe und die Quelle von OCI-Images.

Die Grenzen aus ADR-0020 gelten unverändert weiter: Keine Einstellung schwächt die drei
Garantien aus [`SECURITY.md`](../SECURITY.md#1-the-claim), ein Projekt senkt nie die
Isolationsstufe, eine unsichere Einstellung wird mit einem `Diagnostic` abgelehnt, und kein
Backend weicht stillschweigend auf ein anderes aus.

Der Spike HUM-442 hat den Ersatz auf Debian forky/sid gemessen (Kernel 7.3.4, QEMU 11.1.3,
virtiofsd 1.14.0, vhost-device-vsock 0.3.0, bubblewrap 0.13.1). Der Gast lief mit root und ohne
jeden Filter, also gilt jedes Ergebnis unten auch für einen Angreifer, dem der Gast gehört:

| # | Frage | Ergebnis |
|---|---|---|
| Q1 | Hat der Gast ein Netzwerk-Interface? | Nur `lo`; `connect` zu `1.1.1.1:443` endet in `ENETUNREACH` |
| Q2 | Erreicht genau ein vsock-Port den Host? | CID 2, Port 5000 erreicht den Unix-Socket `<uds>_5000` und bekommt die Antwort des Hosts; Port 5001 und 22 auf CID 2, CID 1 und CID 4 laufen ohne Verbindung in eine Zeitüberschreitung |
| Q3 | Welche Kanäle hält der VMM? | `/dev/kvm`, die Deskriptoren der KVM-VM und der vCPUs, eventfds, io_uring, ein signalfd und die Unix-Sockets der vhost-user-Geräte (in der endgültigen Form fünf für drei Geräte), neben Standardausgabe und Standardfehler; in den Messläufen außerdem die Standardausgabe als serielle Konsole, die die endgültige Form (Q8) weglässt; kein `/dev/vhost-vsock`, kein `/dev/net/tun`, kein tap |
| Q4 | Erreicht ein Projektordner, der nach der Prüfung ausgetauscht wird, den Gast? | Nein: virtiofsd, in bubblewrap mit `--bind-fd` auf dem geöffneten Ordner gestartet, liefert den geprüften Ordner aus (dieselbe Inode 377185), während der Pfad schon einen anderen Ordner nennt |
| Q5 | Kommen Inode-Nummern durch virtiofs? | Ja, für einen Eintrag und für die Wurzel der Einhängung (376298 auf Host und Gast); die Gerätenummer unterscheidet sich (43 auf dem Host, 28 oder 30 im Gast) |
| Q6 | Kann root im Gast nach dem Start Kernel-Module laden? | Nein: Das Init lädt die drei vsock-Module komprimiert aus `/usr/lib/modules` des Hosts und setzt dann `kernel.modules_disabled = 1`; ein späteres `finit_module` scheitert mit `EPERM` |
| Q7 | Laufen alle Prozesse unter einer gehärteten vorübergehenden Nutzer-Unit? | Ja, in der endgültigen Form, mit `PrivateNetwork`, `PrivateUsers`, `NoNewPrivileges`, einer leeren Capability-Begrenzungsmenge, `ProtectSystem=strict`, `ProtectHome=read-only` und `RestrictAddressFamilies=AF_UNIX AF_NETLINK`; darin läuft jeder von QEMU, beiden virtiofsd und vhost-device-vsock in einem eigenen bubblewrap-Gefängnis ohne Netzwerk, und QEMU sieht aus `/boot` nur die Kernel-Datei |
| Q8 | Wie lange dauert ein Start? | In der endgültigen Form ohne Konsole, drei Läufe: 2,20 s, 2,24 s und 2,16 s vom Aufruf von `systemd-run` bis zur ersten Verbindung des Gasts an der Tür (davon 2,04 s, 2,03 s und 2,03 s ab dem Start von QEMU), Laden und Sperren der Module und Einhängen von `/work` eingeschlossen (generischer Kernel von Debian, SeaBIOS); dieselbe Art Unit mit einem bubblewrap-Gefängnis, das `true` startet, braucht 39 bis 43 ms |
| Q9 | Kann ein Gast einen anderen Gast erreichen? | Gemessen unter der Unit aus Q7. Nicht, wenn jeder Gast seinen eigenen Prozess vhost-device-vsock hat: Ein Gast, der einen lauschenden zweiten Gast anwählt, läuft in eine Zeitüberschreitung, selbst mit demselben Gruppennamen. Mit einem Prozess für beide Gäste in einer Gruppe gelingt die Verbindung (Positivkontrolle), Gruppen wirken also nur innerhalb eines Prozesses |
| Q10 | Kann der Host den Steuerkanal öffnen? | Ja: Der Host verbindet sich mit `<uds>`, schreibt `CONNECT 6000` und bekommt `OK 6000` von einem Gast, der auf vsock-Port 6000 lauscht; Daten fließen in beide Richtungen. Der Gast öffnet diesen Kanal nicht |

Ob gehostete CI-Runner KVM anbieten, ist offen (Q11). Befehle und Ausgaben stehen in `backlog/sprint-7.md` unter HUM-442.

## Entscheidung

Wir fügen einen Adapter `qemu` hinter dem vorhandenen Port `SandboxBackend` hinzu. Er führt jede
Sitzung als QEMU-Micro-VM (`-M microvm`) mit dem eigenen Kernel des Hosts aus, gebaut nur aus
Debian-Paketen: `qemu-system-x86`, `virtiofsd`, `vhost-device-vsock` und `bubblewrap`. Kein neuer
Port, keine Abstraktion über QEMU: Die Namen `qemu`, `virtiofsd`, `vhost-device-vsock` und
`microvm` kommen nur in `daemon/crates/sandbox/src/qemu/` vor, bewacht von einem neuen Abschnitt
in `tools/coupling-baseline.toml`.

**Auswahl.** `sandbox.backend` bekommt den Wert `qemu`, Isolationsstufe `kernel`. Die Laufzeit
`krun` von `sandbox.podman.runtime` entfällt; eine Konfiguration, die sie nennt, wird mit einem
`Diagnostic` abgelehnt, dessen Fix `sandbox.backend = "qemu"` nennt. Sie war nie zugelassen, also
hing keine Sitzung von ihr ab. `auto` bleibt bubblewrap. `sandbox.min_isolation = "kernel"`
erfüllt nur `qemu`.

**Prozesse.** Je Sitzung eine vorübergehende Unit des Dienstverwalters des Nutzers (Nachtrag zu
ADR-0010, HUM-434), gehärtet wie in Q7. Darin vier Prozesse, jeder in einem bubblewrap-Gefängnis
mit `--unshare-all`, `--cap-drop ALL` und nur den Pfaden, die er braucht:

- **virtiofsd für die Wurzel**, nur lesend, liefert `/` seines eigenen Gefängnisses aus. Das
  Gefängnis entsteht aus demselben Plan wie beim bubblewrap-Backend (die Einhängungen des
  Profils, der Shim, die CA), also ist die Wurzel des Gasts der Plan und keine Kopie.
  Sein Socket kommt als Deskriptor an (`--fd`), also gehört das Laufzeitverzeichnis der Sitzung
  nicht zur Wurzel des Gasts.
- **virtiofsd für `/work`**, mit dem geprüften Projektordner, per Deskriptor gebunden
  (`--bind-fd`, Q4), und `--sandbox none`, weil das Gefängnis die Sandbox ist.
- **vhost-device-vsock**, ein Prozess je Sitzung und nie zwischen Sitzungen geteilt, weil seine
  Gruppen Gäste nur innerhalb eines Prozesses trennen (Q9); mit einer eigenen Gruppe und
  `--uds-path` im Verzeichnis der Sitzung, nie `--forward-cid`.
- **QEMU** mit `-nodefaults -no-user-config -nic none`, ohne serielle Schnittstelle, Monitor
  oder Anzeige, mit
  `-sandbox on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny`,
  gemeinsamem Gastspeicher über `memory-backend-memfd` (vhost-user braucht ihn), der Kernel-Datei
  und sonst nichts aus `/boot` und `/dev/kvm` als einzigem Gerät.

**Das Ende des Daemons beendet die Sitzung.** Die Unit ist an den Daemon gebunden (`BindsTo` auf
die Unit des Daemons, oder außerhalb einer Unit eine Lebensleine auf dem Host, deren Schreibende
nur der Daemon hält). Root im Gast kann das Ende des Steuerkanals übergehen, also ist die
Lebensleine im Gast nur eine zusätzliche Verteidigungslinie.

**Der Host prüft den VMM.** Neben den `CHECK`-Zeilen aus dem Gast prüft der Daemon vom Host aus,
bevor der Agent startet, und lehnt die Sitzung sonst mit `SANDBOX_047` ab: Jeder Prozess der
Unit hat in seinem Netzwerk-Namespace nur `lo`, QEMU hält nur die Deskriptoren aus Q3 ohne
Konsole, kein `/dev/vhost-vsock` und kein `tun`, das Laufzeitverzeichnis enthält nur die
angegebenen Sockets, und genau ein Prozess vhost-device-vsock mit einem Gerät gehört zur Sitzung.

**Die Tür.** vhost-device-vsock bildet eine Verbindung aus dem Gast zu CID 2, Port P auf den
Unix-Socket `<uds>_P` ab. Der Daemon legt genau einen solchen Socket an, für den Proxy, also
führt genau ein Port irgendwohin (Q2). Der Steuerkanal aus HUM-443 geht in die andere Richtung:
Der Daemon verbindet sich mit `<uds>` und nennt den Port des Shims, also kann der Gast ihn nicht
öffnen (Q10). Kein anderer Socket existiert im Verzeichnis, und das Verzeichnis gehört allein der
Sitzung.

**Der Gast.** Der Shim ist PID 1 (`init=`). Vor allem anderen lädt er `vsock`,
`vmw_vsock_virtio_transport_common` und `vmw_vsock_virtio_transport` aus den Modulen des Hosts
(komprimiert, über `finit_module` mit `MODULE_INIT_COMPRESSED_FILE`) und setzt
`kernel.modules_disabled = 1` (Q6). Danach lauscht er auf `127.0.0.1:3128` und leitet jede
Verbindung an die vsock-Tür weiter, hängt `/work` ein, vergleicht die Einhängetabelle mit dem
Plan, installiert denselben seccomp-Filter wie unter den anderen Backends und startet den
Agenten. Für den Agenten ändert sich nichts.

**Der Kernel.** Der Gast bootet den laufenden Kernel des Hosts (`/boot/vmlinuz-$(uname -r)`) mit
dessen Modulen aus `/usr/lib/modules`, also kommen Sicherheitsupdates des Kernels über die
Distribution. Wo das Image für den Nutzer nicht lesbar ist (Ubuntu installiert es mit Modus
`0600`), ist die Stufe nicht verfügbar, und `humanitl doctor` nennt den Grund; ein Schlüssel
`sandbox.qemu.kernel` nimmt ein anderes Image samt dessen Modulverzeichnis.

**Zulassung.** Wie in ADR-0020: Die Stufe wird mit `SANDBOX_048` abgelehnt, bis ihre
Einhänge-Prüfungen und die Escape-Suite gegen sie in der CI grün sind (HUM-445, HUM-446); HUM-449
lässt sie zu.

## Begründung

- Jede Komponente ist ein Debian-Paket mit eigenen Sicherheitsupdates. Der Daemon baut nichts und
  lädt nichts herunter; `humanitl doctor` prüft, dass die Pakete installiert sind.
- Die Garantien hängen nicht mehr am Gast. Root im Gast besitzt einen Kernel ohne Netzwerkgerät
  und mit abgeschaltetem Laden von Modulen; sein einziger Weg hinaus ist vsock zu CID 2, wo nur
  die Ports, die der Daemon gewählt hat, zu einem Socket führen. Q1, Q2 und Q6 haben das mit root
  im Gast gemessen.
- vhost-device-vsock hält das vsock-Gerät aus dem Kernel des Hosts heraus: kein Modul
  `vhost-vsock`, kein globaler CID-Raum, den andere VMs oder Dienste des Hosts teilen. Seine
  Reichweite ist das Verzeichnis der Sitzung.
- Die Wurzel des Gasts kommt aus demselben Plan wie unter bubblewrap. Die Einhänge-Prüfungen, das
  Festhalten per Deskriptor und die Profile bleiben ein Codepfad, und der Gast sieht, was der
  Plan sagt (Q4, Q5).
- Jeder Prozess läuft in einem bubblewrap-Gefängnis innerhalb einer gehärteten Unit. Eine
  Schwachstelle in QEMU, virtiofsd oder vhost-device-vsock erreicht einen Prozess ohne Netzwerk,
  ohne Capabilities und nur mit seinen eigenen Pfaden, bevor sie das Konto des Nutzers erreicht.

## Verworfene Alternativen

- **Podman mit `krun`** (ADR-0020). libkrun ist in Debian nicht paketiert; die Stufe hätte auf
  jedem Rechner einen Build von libkrun, libkrunfw und crun aus fremden Quellen gebraucht.
- **systemd-vmspawn.** Paketiert und rootless, aber sein vsock nutzt `vhost-vsock` des
  Host-Kernels mit dem globalen CID-Raum, also erreicht der Gast jeden vsock-Dienst des Hosts,
  und es erwartet systemd im Gast mit mehreren Kanälen (Journal, Benachrichtigung,
  Zugangsdaten).
- **crosvm.** Paketiert, mit einem stärkeren Prozessmodell (ein eingesperrter Prozess je Gerät),
  aber Debian liefert einen Git-Schnappschuss aus, und ob seine Gefängnisse ohne root
  funktionieren, ist offen. Es bleibt der Ausweg, falls sich die Angriffsfläche von QEMU als zu
  groß erweist.
- **gVisor (`runsc`).** Ein Kernel im Nutzerraum, kein Gastkernel; es wäre eine Stufe zwischen
  `namespaces` und `kernel`, nicht die Kernel-Stufe.
- **Virtuelle Maschinen von Incus.** Brauchen einen Daemon, der als root läuft, und einen Agenten
  im Gast als zweiten Kanal.
- **virtio-serial statt vsock.** In QEMU und den Gastkernel eingebaut, aber ein Bytestrom je
  Port; jede Proxy-Verbindung bräuchte auf beiden Seiten ein eigenes Multiplex-Protokoll.

## Konsequenzen

- Ein neuer Adapter `daemon/crates/sandbox/src/qemu/` und ein neuer Abschnitt `[qemu]` in
  `tools/coupling-baseline.toml`. Der Podman-Adapter verliert die Laufzeit `krun` und ihre
  Codepfade.
- Der Shim bekommt einen Gastmodus: Laden und Sperren der Module, die Weiterleitung von TCP zu
  vsock und den Steuerkanal als Datenstrom (HUM-443). Er ist PID 1 eines Kernels, also beendet
  eine Panic in ihm die VM, und genau das ist die Absicht der Lebensleine.
- `humanitl doctor` prüft je Voraussetzung: `/dev/kvm` lesbar und schreibbar, die vier Programme
  und die Optionen, die diese Entscheidung nutzt (aus ihrer Hilfe, nicht aus Versionsnummern),
  das Kernel-Image lesbar, das Modulverzeichnis dieses Kernels vorhanden, bubblewrap mit
  `--bind-fd`.
- Die vertrauenswürdige Rechenbasis der Stufe ist KVM im Kernel des Hosts, die Micro-VM-Maschine
  von QEMU mit ihren virtio- und vhost-user-Frontends, virtiofsd, vhost-device-vsock, bubblewrap
  und der Shim. `docs/SECURITY.md` bekommt einen Abschnitt für die Stufe und
  `docs/THREAT-MODEL.md` K-Einträge für den VMM, die vsock-Tür, den Steuerkanal, virtiofs und den
  Gastkernel, jeweils mit dem Code, den sie beschreiben.
- Ein Start kostet etwa zwei Sekunden mehr als unter bubblewrap (Q8).
- Die CI braucht KVM und die Debian-Pakete. Geplant ist ein Job in einem Container `debian:sid`
  auf einem gehosteten Runner, an den `/dev/kvm` durchgereicht wird (HUM-465); bietet kein
  gehosteter Runner KVM, bleibt die Stufe unzugelassen, und der Eigentümer entscheidet über den
  Ersatz (HUM-445).
- Die Stufe gibt es vorerst nur für Linux auf x86-64. macOS kann denselben Gast unter
  Virtualization.framework tragen, das eigene vsock- und virtiofs-Geräte hat; das ist ein
  späterer Adapter und eine spätere Entscheidung.
- vhost-device-vsock ist in Debian testing und unstable paketiert, noch nicht in einer stabilen
  Veröffentlichung und nicht in Ubuntu 24.04. Bis es das ist, ist die Stufe dort nicht
  verfügbar, und `humanitl doctor` sagt, welches Paket fehlt.

## Betroffene Issues

`HUM-442` (der Spike und diese Entscheidung), `HUM-443` bis `HUM-449` (`backlog/sprint-7.md`,
für diesen Adapter neu geplant), `HUM-145` (die Kopplungs-Sperrklinke), `HUM-263` (der
festgehaltene Projektordner), `HUM-350` (die Wegwerf-Sandbox). ADR-0020 bekommt einen Nachtrag,
der für die Kernel-Stufe hierher zeigt.
