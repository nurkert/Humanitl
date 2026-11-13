---
source: adr/0020-podman-sandbox-backend.md
source-sha256: 36b2a5e500e5e457ab6d50a8e282db6a91909bb66ffb5c029854c730638f30c7
lang: de
---
# ADR-0020 · Podman als wählbares zweites Sandbox-Backend unter Linux

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-11-11

## Kontext

Unter Linux ist die Sandbox bubblewrap (ADR-0002). Namensräume, der seccomp-Filter des Shims
und der Kernel des Hosts bilden die ganze Grenze: Ein Kernel-Exploit aus der Sandbox heraus
erreicht den Host unmittelbar. Der Eigentümer hat am 2026-11-11 entschieden, dass Humanitl
nach 0.1.0 Podman als zweites Backend bekommt, das der Nutzer neben bwrap wählt, zuerst als
schlichten rootless Container und später mit einer Kernel-Grenze über eine Micro-VM-Laufzeit.
bwrap bleibt die Vorgabe.

Dieselbe Entscheidung setzt zwei Grenzen, die jeder Teil des Entwurfs einhalten muss:

1. **Keine Einstellung schwächt die drei Garantien** aus
   [`SECURITY.md`](../SECURITY.md#1-the-claim): kein Netzwerk-Interface, genau eine Tür zum
   Proxy, keine neuen Türen. Die eigene Konfiguration eines Projekts kann die
   Isolationsstufe nie senken (`backlog/CONVENTIONS.md` 4.11).
2. **Eine unsichere Einstellung wird mit einem `Diagnostic` abgelehnt** (ADR-0012), beim
   Laden und beim Start. Sie wird nie stillschweigend angewandt, und ein Backend, dem
   Voraussetzungen fehlen, weicht nie stillschweigend auf ein anderes aus.

Abgesehen von diesen Grenzen soll alles einstellbar sein: das Backend global, je
Konfigurationsprofil, je Sitzung auf der Kommandozeile und in der Oberfläche.

Der Port `SandboxBackend` (ADR-0015) hat schon zwei Adapter, bwrap und Seatbelt (ADR-0019),
aber je Build gibt es nur einen davon: `PlatformBackend` ist ein Alias zur Übersetzungszeit,
und Daemon, Kommandozeile und Vorschau greifen am Port vorbei in die Builder-Methoden von bwrap
und in dessen Kommandozeile (`backlog/sprint-5.md`, HUM-292, Abschnitt „Result (desk part,
2026-11-11)"). `tools/coupling-baseline.toml` verzeichnet 33 Dateien außerhalb des Adapters,
die bwrap nennen. Ein zweites Linux-Backend, das der Nutzer zur Laufzeit wählt, kann kein
`cfg`-Zweig sein; es zwingt den Port, neutral zu werden.

Harte Randbedingungen: rootless, kein Dienst, der als root läuft, Debian und Ubuntu als
wichtigste Ziele, der Daemon läuft als gehärtete systemd-Nutzer-Unit (ADR-0010), der Shim
(`humanitl-shim`) ist die einzige Komponente in der Sandbox und bleibt ohne tokio.

## Entscheidung

Wir fügen einen Podman-Adapter hinter dem vorhandenen Port `SandboxBackend` hinzu und machen
den Port zuerst neutral gegenüber den Backends. Kein neuer Port, keine Abstraktion über
Podman: Die Namen `podman`, `crun`, `krun`, `libkrun` und `conmon` kommen nur in
`daemon/crates/sandbox/src/podman/` vor, bewacht von einem neuen Abschnitt in
`tools/coupling-baseline.toml`.

**Auswahl.** Ein neuer Konfigurationsschlüssel `sandbox.backend` mit den Werten `auto`,
`bwrap`, `podman` und, nur unter macOS, `seatbelt`. `auto` heißt bwrap unter Linux und
Seatbelt unter macOS. Er wird in `config.toml` gesetzt, in einem Konfigurationsprofil
(`[config.sandbox]`), je Sitzung mit `--backend` bei `humanitl run` und `humanitl sandbox run`
und in der Oberfläche. Der Schlüssel ist `x-project-scope = "denied"`. Das vorhandene Feld
`sandbox.backend` eines Sandbox-Profils (`profiles/sandbox/*.toml`) bleibt als Anforderung
bestehen: Nennt es ein Backend und ist die wirksame Wahl ein anderes, wird der Start
abgelehnt.

**Stufen.** Jedes Backend meldet eine Isolationsstufe, eine geordnete Aufzählung
`IsolationLevel`:

| Stufe | Backends | Grenze |
|---|---|---|
| `policy` | `seatbelt` | der macOS-Kernel hält den Prozess an ein Profil |
| `namespaces` | `bwrap`, `podman` mit Laufzeit `crun` | Linux-Namensräume und seccomp, gemeinsamer Kernel des Hosts |
| `kernel` | `podman` mit Laufzeit `krun` | ein Gastkernel in einer Micro-VM, dahinter der VMM-Prozess |

Ein zweiter Schlüssel, `sandbox.min_isolation`, nimmt dieselben Werte und ist der einzige
Backend-Schlüssel, den ein Projekt setzen darf (`x-project-scope = "allowed"`). Nur für
diesen Schlüssel gewinnt der strengste Wert aller Ebenen, also kann ein Projekt die
Untergrenze anheben und nie senken. Ein Start, dessen Backend unter der Untergrenze liegt,
wird abgelehnt.

**Podman, gemeinsame Stufe (`sandbox.podman.runtime = "crun"`).** Ein `podman run` je
Sitzung, rootless, vom Daemon gestartet (unter seiner gehärteten Unit als vorübergehende
Unit des Dienstverwalters des Nutzers, Nachtrag zu ADR-0010, HUM-434); der Shim übernimmt die
Deskriptoren des Plans über einen Übergabe-Socket der Sitzung, der genau eine Verbindung
annimmt, nicht mehr über `--preserve-fds`:

- `--network none`, also hat der Container einen Netzwerk-Namensraum nur mit `lo`;
- der Proxy-Socket als einzelne Datei an denselben Pfad gebunden wie unter bwrap;
- `--cap-drop all`, `--security-opt no-new-privileges`, `--read-only` als Wurzel,
  `--userns keep-id`, `--pids-limit`, `--memory`, `--cpus`, `--log-driver none`,
  `--pull never`, `--rm`, Hostname `sandbox`, `--read-only-tmpfs=false` (keine tmpfs der
  Engine auf `/run`, `/tmp`, `/var/tmp`), `--env-host=false` und `--http-proxy=false`
  (sonst kopiert Podman die Proxy-Variablen des Hosts samt Zugangsdaten in den Container);
- jede Konfigurationsquelle von Podman, die der Daemon nicht schreibt, ist abgeschnitten:
  `CONTAINERS_CONF` nennt eine `containers.conf`, die der Daemon schreibt (mit
  `default_mounts_file` auf eine leere Datei als zweite Verteidigungslinie, leerem
  `hooks_dir` und eigenem `seccomp_profile`), `CONTAINERS_STORAGE_CONF` und
  `CONTAINERS_REGISTRIES_CONF` nennen Dateien, die der Daemon schreibt,
  `CONTAINERS_CONF_OVERRIDE`, `CONTAINER_HOST` und `CONTAINER_CONNECTION` werden aus der
  Umgebung entfernt, und die Zeile `podman run` nennt mit `--hooks-dir` ein leeres
  Verzeichnis und mit `--default-mounts-file` die leere Datei. Erst dieses Flag hält
  Einhängungen wie `/run/secrets` fern, denn Podman liest die
  `~/.config/containers/mounts.conf` des Nutzers zusätzlich zur konfigurierten
  `default_mounts_file` (HUM-454);
- der Shim vergleicht die Einhängetabelle im Container (`/proc/self/mountinfo`) mit dem Plan
  als Multimenge von Einträgen (Einhängepunkt, Typ, Quelle, Optionen, Anzahl), die eigenen
  Einträge der Engine eingeschlossen, bevor er den Agenten startet; jeder zusätzliche,
  fehlende, abweichende oder gestapelte Eintrag lehnt den Start ab. Das gilt auch für bwrap;
- der Shim als PID 1 (kein `--init`), mit demselben seccomp-Filter, denselben fünf
  `CHECK`-Zeilen und demselben Steuerkanal für weitere Shells wie unter bwrap;
- die Umgebung des Agenten über einen geerbten versiegelten memfd, den der Shim liest, nie als
  `-e`-Werte, damit kein Geheimnis in der Prozessliste oder in der gespeicherten
  Konfiguration des Containers steht;
- eine Lebensleine als Pipe, deren schreibendes Ende nur der Daemon hält; endet der Daemon,
  beendet der Shim den Container. Podman löst den Container von dem Prozess, der ihn
  startet, also hat `--die-with-parent` kein unmittelbares Gegenstück.

**Zulassung.** Wählbar ist eine Stufe nur, wenn sie zugelassen ist. Seit HUM-448 ist die
gemeinsame Stufe (`crun`) zugelassen, und `--backend podman` startet sie; `auto` bleibt
bubblewrap. Eine noch nicht zugelassene Stufe (heute `krun`) lehnen
`sandbox.backend = "podman"` mit dieser Laufzeit, `--podman-runtime krun` und die
Oberfläche mit einer Diagnose „not yet admitted" (`SANDBOX_048`) ab; nur die Escape-Suite und
die Tests können sie starten, über ein Argument im Code, nicht über einen
Konfigurationsschlüssel; dieser Weg wird nur in den Starter der Escape-Suite und in die Tests
übersetzt, nie in den Daemon oder die Kommandozeile. Die gemeinsame Stufe hat HUM-448
zugelassen, nachdem die Einhänge-Prüfungen (HUM-437) und die Escape-Suite gegen Podman
(HUM-435) grün waren; die Kernel-Stufe wird ebenso nach HUM-445 und HUM-446 zugelassen.

**Podman, Kernel-Stufe (`sandbox.podman.runtime = "krun"`).** Derselbe Adapter mit der
OCI-Laufzeit `krun` (crun, gebaut mit libkrun). Der Gast bekommt kein Netzwerkgerät und keine
transparente Socket-Weiterleitung. Die Tür ist genau ein vsock-Port, den libkrun auf den
Unix-Socket der Sitzung auf dem Host abbildet; die Seite des Hosts nutzt nie `vhost-vsock`.
Terminal, weitere Shells und die Lebensleine laufen über einen zweiten, deklarierten
vsock-Port zu einem Socket des Daemons, weil Deskriptoren nicht in eine VM gelangen. Die
Garantien sind so formuliert, dass sie auch mit root im Gast gelten; die Prüfungen im Gast
werden zur zusätzlichen Verteidigungsschicht, und eine Prüfung des VMM-Prozesses auf der
Seite des Hosts trägt sie. Diese Stufe ist gesperrt: Sie wird erst ausgeliefert, wenn die
Spike-Messungen K1 bis K3 und die Escape-Suite gegen sie in der CI grün sind. Kann crun
„keine Weiterleitung, ein vsock-Port" nicht ausdrücken, ist der Ausweg libkrun direkt aus dem
Adapter, gestartet innerhalb des vorhandenen bwrap-Gefängnisses; das wäre eine
Folgeentscheidung, die hier festgehalten würde.

**Die Tür je Stufe.**

| Backend | Tür | Was der Agent sieht |
|---|---|---|
| bwrap | Proxy-Socket als eine Datei gebunden | `127.0.0.1:3128`, bedient vom Shim |
| Seatbelt | Proxy-Socket plus ein Loopback-Port je Sitzung | den Port der Sitzung |
| Podman `crun` | Proxy-Socket als eine Datei gebunden | `127.0.0.1:3128`, bedient vom Shim |
| Podman `krun` | ein vsock-Port, abgebildet auf den Proxy-Socket der Sitzung | `127.0.0.1:3128`, bedient vom Shim |

**Form der Konfiguration.** Alle Schlüssel haben eine Vorgabe und eine Sichtbarkeitsstufe
(ADR-0011), alle sind `x-project-scope = "denied"` außer `sandbox.min_isolation`:

```toml
[sandbox]
backend = "auto"              # auto | bwrap | podman | seatbelt (macOS)
min_isolation = "policy"      # policy | namespaces | kernel; strictest layer wins;
                              # "namespaces" is an explicit Linux preference

[sandbox.podman]
program = ""                  # empty: podman from PATH
runtime = "crun"              # crun | krun
rootfs = "host"               # host | image
image = ""                    # name@sha256:digest, required for rootfs = "image"
memory_mib = 4096
cpus = 2.0
pids_limit = 1024
start_timeout_secs = 30
```

Es gibt keinen Schlüssel für zusätzliche Podman-Argumente, für einen Netzwerkmodus, für
zusätzliche Capabilities, Geräte oder Volumes außerhalb des Sandbox-Profils und keinen
Schalter für die Weiterleitung der Micro-VM. Ein Wert, der einen davon bräuchte, existiert
nicht, also lässt er sich nicht setzen.

**Neutraler Port.** `SandboxBackend::plan` nimmt einen `PlanRequest` mit Profil, Sitzung und
`LaunchOptions` (den Optionen, die heute Builder-Methoden von `BwrapBackend` und
`SeatbeltBackend` sind); das Backend liefert außerdem eine `PlanPreview` ohne
Nebenwirkungen mit der genauen Kommandozeile, einer neutralen Richtlinientabelle
(verallgemeinert aus `PolicyRow` von Seatbelt), der Tür und den Grenzen. Daemon und
Kommandozeile halten statt des Alias `PlatformBackend` ein Enum `Backend` mit einer Variante je
Adapter. Die Skizze steht im Ergebnis von HUM-292; HUM-428 setzt sie um.

## Begründung

Podman läuft rootless ohne einen Dienst, der als root läuft, es ist in Debian und Ubuntu
paketiert, und es liegt auf den Ubuntu-Runnern von GitHub, also kann die Escape-Suite in der
CI ohne eigenen Runner gegen es laufen. `--network none` ergibt denselben leeren
Netzwerk-Namensraum wie `--unshare-all`, also ist Garantie 1 wieder eine Eigenschaft des
Kernels und keine Konfiguration; Garantie 2 ist wieder eine gebundene Socket-Datei;
Garantie 3 ist dasselbe Shim-Binary mit demselben Filter auf demselben Kernel. Die gemeinsame
Stufe braucht deshalb keine neu formulierten Garantien, nur einen zweiten Weg, dieselben
Einhängungen auszudrücken. Widerlegen lässt sie sich mit demselben Lauf von `tests/escape/`,
bei dem nur das Backend umgestellt ist.

Was Podman gegenüber bwrap hinzufügt, ist der Weg über Images: Eine Sitzung kann auf einem
OCI-Image laufen, in dem eine Toolchain oder ein Agent installiert ist, und derselbe Adapter
erreicht eine Kernel-Grenze, indem er die OCI-Laufzeit wechselt. Die Kernel-Stufe ist der
Grund, warum der Eigentümer überhaupt ein zweites Backend will; die gemeinsame Stufe ist der
Schritt, der den Port neutral macht und den Adapter erprobt, bevor eine VM ihre eigenen
Fragen aufwirft.

Die Isolationsuntergrenze als eigener Schlüssel hält beide Grenzen mechanisch: Der
Backend-Schlüssel wird nie aus einem Projekt gelesen, und der eine Schlüssel, den ein Projekt
setzen darf, kann sich nur in eine Richtung bewegen. Ein Test je Richtung belegt das.

Die Umgebung über einen Deskriptor statt `-e`, der Log-Treiber `none` und die Lebensleine
folgen aus dem Aufbau von Podman, nicht aus Geschmack: Podman schreibt die Konfiguration des
Containers samt jedem `-e`-Wert auf die Platte, protokolliert die Ausgabe des Containers als
Vorgabe und lässt den Container unter `conmon` laufen, das nicht mit dem Prozess endet, der
ihn gestartet hat.

Wo eine Annahme offen ist, nennt das Ergebnis von HUM-292 die Messung und den Befehl; die
Issues in `backlog/sprint-7.md` beginnen erst, wenn die Messungen P1 bis P9 der gemeinsamen
Stufe festgehalten sind.

## Verworfene Alternativen

- **Incus.** Seine VMs und Systemcontainer laufen unter `incusd` als root, und eine VM hat
  neben jeder Tür einen Kanal zum Agenten, also gäbe es zwei Kanäle und einen privilegierten
  Dienst.
- **microsandbox.** Dieselbe Micro-VM-Technik wie libkrun, aber mit eigenem Serverprozess
  und einem dauerhaft offenen Konsolenkanal zu einem Agenten im Gast, bei der letzten Prüfung
  noch Beta (BACKLOG.md 9.12).
- **Kata Containers.** Eine VM je Container, aber meist hinter einem containerd, der als root
  läuft, mit `vhost-vsock` auf der Seite des Hosts, über das ein Gast andere vsock-Listener
  des Hosts erreicht.
- **gVisor (`runsc`).** Braucht kein `/dev/kvm`, hat aber auch keinen Gastkernel, und wie gut
  es mit beliebigen Toolchains von Agenten zurechtkommt, ist schwer vorherzusagen; vorgemerkt
  als Kandidat für Hosts ohne KVM, nicht als Kernel-Stufe.
- **Docker.** Ein Daemon als root, dessen Socket eine Root-Shell auf dem Host ist
  (ADR-0002); die rootless Variante bringt nichts, was rootless Podman nicht schon hat, und
  ihr fehlt der Vorteil der Paketierung.
- **Ein zweites Linux-Backend als `cfg`-Zweig wie Seatbelt.** Billig für einen Build, aber der
  Nutzer soll je Sitzung umschalten, also muss die Wahl zur Laufzeit fallen.
- **Nichts tun.** bwrap bleibt unter Linux die einzige Grenze; der Wunsch des Eigentümers
  nach einer Kernel-Grenze bleibt offen.

## Konsequenzen

- HUM-428 macht den Port neutral, bevor irgendein Podman-Code landet: `PlanRequest`,
  `LaunchOptions`, `PlanPreview`, `IsolationLevel`, das Enum `Backend`, und Vorschau und
  Einhängetabelle entstehen aus dem Plan, statt aus bwrap-Flags geparst zu werden. Die
  Kopplungszahlen in `tools/coupling-baseline.toml` sinken.
- `tests/escape/` wird nach Backend parametrisiert (`HUMANITL_ESCAPE_BACKEND`), und ein
  Backend ist nur zulässig, wenn die ganze Suite gegen es in der CI grün ist. Die
  Kernel-Stufe fügt Proben mit root im Gast und eine Prüfung des VMM auf der Seite des Hosts
  hinzu.
- Die Nutzer-Unit des Daemons (`packaging/systemd/humanitld.service`) blockiert rootless
  Podman in ihrer jetzigen Form: `PrivateUsers`, `ProtectSystem=strict` ohne den
  Container-Speicher, `ProtectControlGroups` und `NoNewPrivileges` mit `newuidmap`. Welche
  Härtung bleibt, ist Messung P8; jede Lockerung steht im selben Commit in
  `docs/SECURITY.md` „Service hardening".
- Die Sicherheitsdokumente bekommen einen Abschnitt zu Podman und neue K-Einträge in
  `docs/THREAT-MODEL.md` für die Container-Engine und, in der Kernel-Stufe, für den VMM, die
  vsock-Tür, den Steuerkanal, virtiofs und den Gastkernel, jeweils im Commit des Codes, den
  sie beschreiben.
- Die Konfiguration wächst um eine Gruppe, `sandbox.podman`, und das Protokoll um einen RPC,
  der die Backends mit Stufe, Verfügbarkeit und fehlenden Voraussetzungen auflistet
  (Parität nach ADR-0018: CLI `humanitl sandbox backends`).
- Neues Risiko: `containers.conf`, `mounts.conf`, `storage.conf` und Hook-Verzeichnisse des
  Nutzers können jedem Container Einhängungen (Geheimnisse des Hosts unter `/run/secrets`),
  Geräte oder Hooks hinzufügen. Der Adapter startet Podman für jede Quelle mit eigenen
  Konfigurationsdateien, und die Prüfungen des Shims von innen behalten das letzte Wort: Ein
  zusätzlicher Einhängepunkt, ein Interface oder ein Socket hält die Sitzung an, bevor der
  Agent startet.
- Neues Risiko: Die Kernel-Stufe hängt von libkrun ab, das Debian nicht paketiert. Bis es das
  tut, ist die Stufe experimentell, und `humanitl doctor` sagt, was fehlt.

**Nachträge, die diese Entscheidung braucht**, geschrieben, als die Issues landeten: die Unit
in ADR-0010 mit HUM-434, der Rest mit HUM-441:

- **ADR-0002.** Aus „Docker später als zweites Backend" wird „Podman als zweites
  Linux-Backend (ADR-0020)"; die Kommandozeile als lesbare Richtlinie bleibt, aber die
  Tabelle, die die Oberfläche zeigt, kommt aus dem neutralen Plan.
- **ADR-0010.** Das `.deb` führt `podman` (und später die krun-Laufzeit) unter `Recommends`,
  nie unter `Depends`; die Nutzer-Unit bekommt die Härtung, die laut P8 rootless Podman
  braucht, oder einen dokumentierten Weg, Podman außerhalb von ihr zu starten.
- **ADR-0015.** Die Port-Tabelle nennt `bwrap`, `seatbelt` und `podman` als Adapter von
  `SandboxBackend`; `docker` und `microvm` als spätere Adapter werden durch `podman` ersetzt
  (mit den Laufzeiten `crun` und `krun`).

## Nachtrag 2026-11-13: Die Kernel-Stufe wandert in ADR-0021 (HUM-442)

Die Kernel-Stufe dieser Entscheidung, Podman mit der Laufzeit `krun`, wird ersetzt. libkrun ist
in Debian nicht paketiert, und der Eigentümer hat entschieden, dass die Stufe nur aus Paketen
der Distribution gebaut wird. ADR-0021 entscheidet über den Ersatz: einen eigenen Adapter
`qemu`, eine QEMU-Micro-VM mit virtiofsd und vhost-device-vsock, mit den Messungen des Spikes
HUM-442. Alles, was diese Entscheidung über die gemeinsame Stufe (`crun`), die Auswahl, die
Stufen, `sandbox.min_isolation` und die Zulassung sagt, bleibt in Kraft. Wo sie `krun`,
`libkrun` oder die Kernel-Stufe von Podman nennt, gilt stattdessen ADR-0021; die Laufzeit `krun`
verschwindet mit HUM-464 aus `sandbox.podman.runtime`.

## Betroffene Issues

`HUM-292` (der Spike und sein Ergebnis, `backlog/sprint-5.md`), `HUM-428` bis `HUM-449`
(`backlog/sprint-7.md`), `HUM-145` (die Kopplungs-Sperrklinke), `HUM-263` (der festgehaltene
Projektordner), `HUM-350` (die Wegwerf-Sandbox), `HUM-448` (die gemeinsame Stufe zugelassen,
Status Accepted), `HUM-441` (die Nachträge zu ADR-0002, ADR-0010 und ADR-0015), `HUM-442`
(die Kernel-Stufe wandert in ADR-0021, Nachtrag).
