# ADR-0002 · bubblewrap zuerst, Docker später als zweites Backend
Status: Accepted
Datum: 2026-09-02

## Kontext

Der Agent muss in einer Umgebung laufen, in der es physisch keinen Weg ins Netz
gibt außer dem einen Socket, den Humanitl kontrolliert. Das ist Prinzip 1:
Physik statt Vertrauen. Ein Agent, der `HTTP_PROXY` ignoriert oder eine eigene
DNS-Auflösung mitbringt, darf nicht durchkommen, sondern muss scheitern
(fail-closed).

Daraus folgen drei Garantien, die live im UI prüfbar sein sollen
(`BACKLOG.md` 4.1):

1. **Kein Netzwerk-Interface.** In der Sandbox existiert nur `lo`. Keine IP,
   kein DNS, kein ICMP, kein QUIC, keine Raw Sockets.
2. **Genau eine Tür.** Der Proxy-Socket des Daemons wird als einzelne Datei in
   die Sandbox gebunden; `find / -type s` liefert genau diese eine Datei.
3. **Keine neuen Türen.** Ein seccomp-Filter verhindert, dass in der Sandbox
   überhaupt noch andere Socket-Familien oder -Typen geöffnet werden können.

Die Sandbox-Technologie muss diese drei Garantien tragen, rootless funktionieren
(die Zielgruppe installiert ein `.deb` und will nicht `sudo` tippen), ohne
Hintergrunddienst auskommen und ihre Policy in einer Form ausdrücken, die das UI
einem Nicht-Experten zeigen kann.

## Entscheidung

Sandbox-Backend im MVP ist [bubblewrap](https://github.com/containers/bubblewrap)
(`bwrap`), angesprochen über den Port `SandboxBackend` in
`daemon/crates/sandbox`. Die Policy ist eine einzige lesbare
`bwrap`-Kommandozeile, die aus einem Profil unter `profiles/sandbox/*.toml`
erzeugt und im UI unverkürzt angezeigt wird („Exakte bwrap-Kommandozeile anzeigen").

Kern der Kommandozeile: `--unshare-all --new-session --die-with-parent
--cap-drop ALL`, dazu `--unshare-pid --unshare-ipc --unshare-uts`, eigenes
`/proc`, `--tmpfs /dev/shm`, Hostname `sandbox`, und eine Mount-Allowlist.
`--cap-drop ALL` steht unbedingt in der Argv, direkt hinter `--new-session`,
und ist nicht konfigurierbar (`backlog/CONVENTIONS.md` 4.11). Nie
gemountet werden `$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `~/.ssh`, `~/.gitconfig`,
`~/.netrc` sowie X11-, Wayland-, dbus- und docker-Sockets. `/work` ist der
Projektordner, wahlweise `ro` oder `rw`; `.git/hooks`, `.git/config`, `.envrc`,
`.vscode` und `.idea` werden mit tmpfs maskiert.

Docker kommt nach dem MVP als zweite Implementierung desselben Traits
(`SandboxBackend`), für vollständige Toolchain-Images. Kern und Anwendung
bleiben dabei unberührt (ADR-0015).

## Begründung

`--unshare-all` gibt der Sandbox einen leeren Netzwerk-Namespace: nur `lo`
existiert, die Routing-Tabelle ist leer. Damit ist Garantie 1 keine
Konfigurationsfrage, sondern eine Kernel-Eigenschaft. Wichtig ist der
Nebeneffekt auf abstrakte Unix-Sockets: Der abstrakte Namespace ist an das
Netzwerk-Namespace gebunden, also ist er in der Sandbox ebenfalls leer. Ein
Agent kann sich damit nicht an einen abstrakten Socket des Hosts hängen.

bwrap läuft rootless über User-Namespaces und braucht keinen Daemon. Die Policy
ist ein `argv`, kein Zustandsobjekt in einem fremden Prozess — sie lässt sich
anzeigen, kopieren, in einem Bugreport mitschicken und außerhalb von Humanitl
reproduzieren. Für ein Werkzeug, dessen Sicherheitsversprechen der Nutzer
nachvollziehen können soll, ist diese Transparenz das entscheidende Argument.

bwrap ist in Debian und Ubuntu paketiert und wird von Flatpak in großem Maßstab
benutzt, ist also weder exotisch noch unbeobachtet.

Die Grenzen werden nicht verschwiegen: bwrap braucht unprivilegierte
User-Namespaces. Auf Ubuntu 24.04 und neuer schränkt ein AppArmor-Profil diese
ein; `humanitl doctor` (HUM-075) prüft `unprivileged_userns_clone` und das
AppArmor-Profil und liefert bei Fehlschlag einen konkreten Fix statt einer
Fehlermeldung.

## Verworfene Alternativen

- **Docker mit `--network none` (im MVP).** Erzeugt zwar ebenfalls ein leeres
  Netzwerk-Namespace, braucht aber trotzdem eine Bridge und einen seccomp-Filter
  im Container, läuft über einen Daemon als root, und `docker.sock` ist ein
  zusätzlicher Fußangel-Pfad: Wer ihn erreicht, ist root auf dem Host. Für die
  Zielgruppe ist außerdem „Docker installieren" eine Hürde vor dem ersten Start.
  Kommt nach dem MVP als zweites Backend, wo Toolchain-Images den Aufwand
  rechtfertigen.
- **gVisor.** Rootless ohne eigenen Netstack unbrauchbar für unser Modell, und
  die Kompatibilitätsfläche für beliebige Agent-Toolchains ist schwer
  abzuschätzen.
- **nsjail.** Technisch nah an bwrap, aber kein Debian-Paket. Das
  Auslieferungsproblem („ein Paket, alles drin") wiegt schwerer als der
  Funktionsunterschied.
- **Firecracker oder Kata Containers.** Stärkere Isolation über KVM, aber
  Tooling-schwer, Kernel-Image im Paket, langsamer Start, KVM-Zugriff nicht
  überall gegeben. Vorgemerkt als v2 für ein stärkeres Threat Model, nicht für
  den MVP.
- **Nur seccomp ohne Namespaces.** Ein seccomp-Filter allein hält kein
  Dateisystem und keine Prozessliste zurück; er ist in unserem Aufbau der
  doppelte Boden (Garantie 3), nicht die tragende Wand.

## Konsequenzen

- Die Sandbox ist Linux-spezifisch. macOS bräuchte ein eigenes Backend
  (`seatbelt`), das der Port `SandboxBackend` erlaubt, aber niemand baut.
- Ein Agent in der Sandbox erreicht den Proxy über `127.0.0.1:3128`, weil im
  leeren Netzwerk-Namespace nur Loopback existiert. Die Übersetzung von diesem
  TCP-Endpunkt auf den Unix-Socket des Hosts leistet der Shim (`humanitl-shim`).
  Das Prozessmodell kommt ohne `--as-pid-1` aus: PID 1 in der Sandbox ist das
  Init von bwrap; der Shim läuft darunter und hält die Brücke selbst ohne
  Filter; nur sein Kind, der Agent, trägt `Seccomp: 2`. Der Isolation-Check
  liest deshalb ein gefiltertes Kind, nie `/proc/1/status`. Details in
  ADR-0016, das die Bridge-Liste verallgemeinert.
- Weil im Namespace nur `lo` existiert, darf seccomp `AF_INET`/`AF_INET6` mit
  `SOCK_STREAM` erlauben, ohne die Garantie zu schwächen. Der Filter verhält
  sich genau so (`backlog/CONVENTIONS.md` 4.10 und 4.11): `socket()` ist nur
  für das Kreuzprodukt `allow_families` × `allow_types` aus dem Profil erlaubt;
  `arg1` wird dabei mit `0xff` maskiert, damit `SOCK_NONBLOCK|SOCK_CLOEXEC`
  durchgehen. Jede andere Familie und jeder andere Typ liefert `EPERM`. Ein
  Arch-Mismatch führt zu `KillProcess`. x32-Syscalls (`nr & 0x40000000`)
  liefern `EPERM` über ein handgeschriebenes BPF-Präludium vor dem
  seccompiler-Programm; das ist Teil der Entscheidung, nicht „falls nötig".
  `socketpair()` bleibt vom Filter unberührt: Es kennt nur `AF_UNIX`, verbindet
  zwei Deskriptoren desselben Prozessbaums und ist kein Egress (Node und Bun
  brauchen es für ihre Kindprozess-IPC). Der Filter liegt in
  `daemon/bin/humanitl-shim/src/seccomp.rs` und führt unter `#[cfg(test)]`
  eine Tabelle aller Regeln; `docs/SECURITY.md` zitiert die Liste von dort.
  Das Profil `browser` erweitert die Familien später um `AF_UNIX` (ADR-0016) —
  die Garantie trägt dann weiter das Netzwerk-Namespace plus die
  Mount-Allowlist.
- bwrap-Version und User-Namespace-Verfügbarkeit werden zur
  Installationsbedingung. `humanitl doctor` prüft beides und bietet den
  Paketbefehl zum Kopieren an; der Sandbox-Screen deaktiviert bei Fehlschlag den
  Start mit konkretem Grund und bietet nie „trotzdem starten" an.
- Die drei Garantien sind testbar und werden getestet: Escape-Test 1
  (Socket-Verweigerung, erwartet `socketpair()` = ok) und 2 (Mount-Oberfläche)
  prüfen sie in der laufenden Sandbox, das Isolation-Panel zeigt dieselben
  Prüfungen live im UI.

## Betroffene Issues

`HUM-010` (Sandbox-Profil-Format), `HUM-011` (bwrap-Launcher), `HUM-012`
(humanitl-shim mit seccomp), `HUM-013` (Proxy-Socket-Bind), `HUM-006`
(Escape-Test-Harness), `HUM-040` (Sandbox-Screen), `HUM-041`
(Isolation-Check-Panel), `HUM-043` (`/work`-Härtung), `HUM-075`
(`humanitl doctor`).
