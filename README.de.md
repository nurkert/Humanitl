# Humanitl

<p align="center">
  <img src="images/readme/icon.png" width="96" height="96" alt="App-Symbol von Humanitl">
</p>

<p align="center"><strong>Humanitl lässt einen KI-Coding-Agenten in einer Sandbox ins Internet, eine freigegebene Anfrage nach der anderen.</strong></p>

<p align="center">
  <a href="https://github.com/nurkert/Humanitl/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/nurkert/Humanitl/actions/workflows/ci.yml/badge.svg"></a>
  <a href="https://github.com/nurkert/Humanitl/releases"><img alt="Neuestes Release" src="https://img.shields.io/github/v/release/nurkert/Humanitl?include_prereleases&sort=semver"></a>
  <a href="LICENSE"><img alt="Lizenz: GPL-3.0-only" src="https://img.shields.io/badge/license-GPL--3.0--only-blue.svg"></a>
  <img alt="Plattform: Linux und macOS" src="https://img.shields.io/badge/platform-Linux%20%7C%20macOS-lightgrey.svg">
</p>

<p align="center">
  <a href="https://humanitl.de/">Website</a> ·
  <a href="https://humanitl.de/download/">Download</a> ·
  <a href="docs/SECURITY.md">Sicherheitsmodell</a> ·
  <a href="README.md">English</a>
</p>

<picture>
  <source media="(prefers-color-scheme: light)" srcset="images/readme/intercept-light.png">
  <img src="images/readme/intercept-dark.png" width="100%" alt="Humanitl, Bereich Intercept: 15 angehaltene Anfragen an registry.npmjs.org, die erste mit allen Kopfzeilen, darunter Allow, Edit + Allow und Block; rechts das Domain-Panel mit Katalogeintrag, registrierbarer Domain und Rang.">
</picture>

<p align="center"><sub>Die macOS-App spielt im Demo-Modus des Daemons ein aufgezeichnetes <code>npm install</code> ab: 15 Anfragen angehalten, keine gesendet. Die Oberfläche im Bild ist englisch; die App gibt es auf Deutsch und Englisch.</sub></p>

Der Agent läuft ohne Netzwerk-Interface (unter macOS gibt es Interfaces, aber jeder Weg nach draußen
außer der Tür wird verweigert). Sein einziger Weg nach draußen ist ein moderierender Proxy auf dem
Host: Eine Anfrage, die keine Regel entscheidet, wird angehalten, mit Kopfzeilen, Parametern und
Body gezeigt und erst nach deiner Freigabe weitergeleitet. Ohne Halt gehen nur Anfragen, die eine
`allow`-Regel trifft, und die erklärte LLM-Durchleitung zum Sprachmodell. Alles wird aufgezeichnet, außer
während eines Ausfalls der Aufzeichnung (Platte voll, Datenbank gesperrt); dann läuft der Verkehr
weiter, und die App sagt es an jeder Karte.

### So funktioniert es in drei Schritten

1. **Der Agent läuft versiegelt.** bubblewrap und seccomp unter Linux, Seatbelt unter macOS; sein
   einziger Weg hinaus ist der Proxy von Humanitl.
2. **Jede unbekannte Anfrage wird angehalten.** Du siehst Methode, Host, Kopfzeilen, Query und Body,
   den Grund des Halts und was die Domain ist.
3. **Du entscheidest.** Freigeben, bearbeiten und freigeben, blockieren oder als Regel merken; jede
   Entscheidung landet in einem Audit-Log mit Hash-Kette.

## Warum

Lokale Sprachmodelle taugen inzwischen für echte Arbeit, wissen aber weniger auswendig und müssen
nachschlagen. Ein Agent, der mit vertraulichen Kundendaten arbeitet, kann diese Daten in eine
Anfrage schreiben, durch Prompt Injection, ein bösartiges Paket oder ein manipuliertes Modell. Die
einzige sichere Antwort war bisher: gar kein Internet. Mit Humanitl fragt der Agent, der Proxy hält
an, ein Mensch entscheidet, und alles wird aufgezeichnet. Humanitl läuft lokal auf deinem Rechner
und sendet keine Daten an uns: kein Konto, keine Telemetrie, keine Absturzberichte.

## Die drei Garantien

Jede lässt sich in der laufenden Sandbox selbst prüfen. Unter Linux:

1. **Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.** `bwrap --unshare-all`: nur `lo`,
   keine Adresse, keine Route, kein DNS, kein ICMP, kein QUIC, keine Raw-Sockets. Prüfung:
   `ip link` zeigt nur `lo`.
2. **Genau eine Tür: ein Socket, der zu Humanitl führt.** Der Proxy-Socket des Daemons ist als
   einzelne Datei in die Sandbox gebunden. Prüfung:

   ```sh
   find / \( -path /proc -o -path /sys -o \( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \) \) -prune -o -xtype s -print 2>/dev/null
   # genau eine Zeile: /run/humanitl/proxy.sock
   ```

   `-xtype s`, nicht `-type s`: bwrap bindet den Socket über eine leere Datei, und ein bloßes `-type` glaubt dem Verzeichniseintrag; kein `-xdev`, weil `/work`, `/tmp`, `/dev/shm` und `/home/agent` eigene Mounts sind, auf denen ein untergeschobener Socket auftauchen muss.

3. **Der Kernel öffnet keine neue Tür (seccomp).** Der Shim setzt vor dem Start des Agenten einen
   Filter: `socket()` nur für `AF_INET`/`AF_INET6` mit `SOCK_STREAM`, alles andere `EPERM`, dazu
   `ptrace`, `io_uring_*`, `process_vm_*`, `keyctl` und jeder x32-Systemaufruf verweigert. Prüfung:
   `grep Seccomp /proc/self/status` zeigt `Seccomp: 2`.

### Unter macOS

macOS kennt keinen Netzwerk-Namespace und kein seccomp. Dort ist die Sandbox Seatbelt: Der Daemon
schreibt je Sitzung ein Profil, und der Kernel hält den Shim und jeden seiner Prozesse daran fest.

| # | Linux | macOS |
|---|---|---|
| 1 | Kein Netzwerk-Interface | **Kein Weg nach draußen.** Die Interfaces gibt es, aber jedes `connect`, `bind` und `sendto` außer zur Tür endet in `EPERM`; es gibt keine Namensauflösung. |
| 2 | Genau eine Tür | **Genau eine Tür.** Erlaubt sind der Proxy-Socket des Daemons und der Loopback-Port der Sitzung, über den der Shim zu ihm weiterreicht. |
| 3 | seccomp | **Der Kernel hält den Prozess am Profil fest.** `AF_ROUTE`, `AF_SYSTEM`, `AF_NDRV` und Raw-Sockets werden schon bei `socket(2)` verweigert. |

Einzelheiten in [`docs/SECURITY.md` Abschnitt 12](docs/SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap).

### Selbst prüfen

```sh
humanitl sandbox check                                                 # Linux
/Applications/Humanitl.app/Contents/MacOS/bin/humanitl sandbox check   # macOS
```

Der Shim misst die drei Garantien bei jedem Start. Schlägt eine fehl, wird die Sitzung gestoppt;
ein „trotzdem starten" gibt es nicht. Die Escape-Tests ESC-1 bis ESC-5 und die Seatbelt-Suite
laufen in der CI bei jedem Commit.

## Wovor es nicht schützt

Lies [`docs/THREAT-MODEL.md`](docs/THREAT-MODEL.md), bevor du dich auf Humanitl verlässt. Fünf
Seitenkanäle sind erklärt, in der App sichtbar und nicht geschlossen
([`docs/SECURITY.md`](docs/SECURITY.md#5-declared-side-channels)):

- **Die LLM-Durchleitung zum Sprachmodell:** Daten können hinaus. Dein Modell sieht jeden Prompt des
  Agenten ohne Halt; dieser Host gehört zu dem, dem du vertraust.
- **Das Projektverzeichnis `/work`:** Daten können hinaus. Es ist beschreibbar; was der Agent dort
  hinterlässt, trägt vielleicht später ein Mensch hinaus.
- **Die Terminalausgabe** kommt vom Agenten und wird gefiltert, nicht geglaubt.
- **Hostnamen** stehen im Klartext in der Aufzeichnung und in jedem Export.
- **Paket-Caches** gelten je Projekt und tragen Daten zwischen den Sitzungen dieses Projekts.

**Und du bist der Klassifikator:** Eine freigegebene Anfrage geht hinaus, also lies, was du
freigibst. Antworten und WebSocket-Frames werden aufgezeichnet, aber nicht angehalten, es gibt kein HTTP/2
zum Ziel, und die Isolationsprüfung läuft, wenn der Agent schon gestartet ist
([Abschnitt 5.6](docs/SECURITY.md#56-other-known-limits)).

### Wo macOS weniger leistet als Linux

Siehe [`docs/SECURITY.md`](docs/SECURITY.md#what-is-different-and-what-follows):

- **K-19, geteiltes Loopback:** Jeder lokale Prozess jedes Kontos erreicht den Loopback-Port der
  Tür und damit den Proxy, mit den Regeln der Sitzung; was eine `allow`-Regel oder die LLM-Durchleitung
  durchlässt, geht ohne Halt hinaus.
- **K-20, kein PID-Namespace:** Ein Prozess, der eine eigene Sitzung startet, überlebt die Sandbox
  und kann, solange der Daemon läuft, weiter Anfragen über den Proxy-Socket stellen. Bis HUM-331
  das schließt: nach einer Sitzung, der du nicht traust, `humanitl daemon restart`.
- **K-21, Sperrliste statt Freigabeliste beim Lesen:** Die Sandbox liest alles außerhalb von
  `/Users`, `/Volumes`, `/private/var/folders` und `/private/tmp`, darunter ganz `/etc`,
  `/opt/homebrew` und `/usr/local/etc`. Zugangsdaten in einer Konfigurationsdatei dort kann der Agent
  lesen und ohne Halt an die LLM-Durchleitung schicken. Geheimnisse gehören ins Home-Verzeichnis.
- **K-22, Seatbelt ist abgekündigt:** Getestet auf macOS 15 (CI) und benutzt auf macOS 27, andere
  Versionen nicht. Eine rote Isolationszeile stoppt die Sitzung auf jeder Version.
- **K-23, Seitenkanäle innerhalb des Hosts:** POSIX-Semaphore und Shared Memory, das Systemlog.
  Keiner davon verlässt den Rechner.

## Installation

Alle Dateien liegen auf der [Release-Seite](https://github.com/nurkert/Humanitl/releases) und auf
[humanitl.de](https://humanitl.de/download/).

**Linux** (Debian 13 oder Ubuntu 24.04 und neuer, x86_64, Kernel 5.4+, unprivilegierte User
Namespaces, bubblewrap 0.8+, systemd-Benutzersitzung), am besten über das APT-Repository:

```sh
curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh
humanitl daemon install
humanitl sandbox check
```

Wer kein Skript in eine Root-Shell leiten will, liest es vorher
(`curl -fsSL https://apt.nurkert.de/install/humanitl | less`) oder nimmt das `.deb` von der
Release-Seite: `sudo apt install ./humanitl_<version>_amd64.deb`, danach `humanitl daemon install`.
Das Archiv `humanitl-<version>-linux-x86_64.tar.gz` ist für Server und Skripte gedacht; sein
`bin/humanitl daemon install` schreibt eine eigene Benutzer-Unit ohne Socket-Aktivierung.

**macOS** (Apple Silicon und Intel, macOS 12 oder neuer): `Humanitl-<version>-macos.dmg` öffnen und
Humanitl nach „Programme" ziehen. Die App ist nur ad hoc signiert; den ersten Start einmal unter
Systemeinstellungen › Datenschutz & Sicherheit › „Dennoch öffnen" erlauben. Alternativ
`xattr -dr com.apple.quarantine /Applications/Humanitl.app`; das hebt die Gatekeeper-Prüfung für
diese App vollständig auf, also nur für eine geprüfte Kopie von der Release-Seite. Der
Einrichtungsbildschirm installiert den Daemon als launchd-Agent.

**Prüfsummen:** `sha256sum -c SHA256SUMS --ignore-missing` (macOS:
`shasum -a 256 -c SHA256SUMS --ignore-missing`). Releases vor 1.0.0 sind **nicht signiert**;
`SHA256SUMS` schützt vor einem beschädigten Download, nicht vor einem manipulierten Release.

Alles Weitere in [`docs/guides/install.md`](docs/guides/install.md).

## Erste Sitzung

1. App öffnen; der Einrichtungsbildschirm prüft Hintergrunddienst, Sandbox und Modellserver.
2. Modellserver eintragen und testen.
3. Den Projektordner wählen, den der Agent sehen darf.
4. Sitzung starten und die Isolationsprüfung lesen: drei grüne Zeilen und eine gelbe für das
   Modell.
5. Die erste angehaltene Anfrage entscheiden: freigeben, blockieren oder als Regel merken.

Die Tour durch die App, die Kommandozeile, die Architektur und den Stand gibt es im
[englischen README](README.md); die übrige Dokumentation ist ebenfalls englisch. Sicherheitslücken
bitte nicht als öffentliches Issue melden, sondern wie in [`SECURITY.md`](SECURITY.md) beschrieben.

## Lizenz

GPL-3.0-only, siehe [`LICENSE`](LICENSE). Domain-Daten aus der Public Suffix List und der Majestic
Million unter CC BY 3.0 (`catalog/RANKS-LICENSE`). Das App-Symbol beruht auf Font Awesome Free
(CC BY 4.0), siehe [`THIRD_PARTY.md`](THIRD_PARTY.md).
