# Humanitl

Humanitl lässt einen KI-Coding-Agenten in einer Sandbox ins Internet, eine freigegebene Anfrage nach
der anderen.

Der Agent läuft ohne Netzwerk-Interface. Sein einziger Weg nach draußen ist ein moderierender Proxy
auf dem Host: Eine Anfrage, die keine Regel entscheidet, wird angehalten, mit Kopfzeilen,
Parametern und Body gezeigt und erst nach deiner Freigabe weitergeleitet. Ohne Halt gehen nur
Anfragen, die eine `allow`-Regel trifft, und die erklärte Durchreiche zum Sprachmodell. Alles wird
aufgezeichnet, außer während eines Ausfalls der Aufzeichnung; dann läuft der Verkehr weiter, und
die Anwendung sagt es an jeder Karte.

## Die drei Garantien

1. **Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.** Prüfung in der Sandbox:
   `ip link` zeigt nur `lo`.
2. **Genau eine Tür: ein Socket, der zu Humanitl führt.** Prüfung:
   `find / \( -path /proc -o -path /sys -o \( -path '/dev/*' ! -path /dev/shm ! -path '/dev/shm/*' \) \) -prune -o -xtype s -print 2>/dev/null`
   findet genau `/run/humanitl/proxy.sock`.
3. **Der Kernel öffnet keine neue Tür (seccomp).** Prüfung: `grep Seccomp /proc/self/status`
   zeigt `Seccomp: 2`.

Offen und sichtbar bleiben zwei Kanäle: das Projektverzeichnis `/work` und die Verbindung zum
Sprachmodell. Alle drei Prüfungen auf einmal: `humanitl sandbox check`.

## Installation

```sh
# Debian 13 / Ubuntu 24.04
sudo apt install ./humanitl_<version>_amd64.deb
humanitl daemon install

# AppImage
chmod +x Humanitl-<version>-x86_64.AppImage
./Humanitl-<version>-x86_64.AppImage --cli daemon install

humanitl daemon status
humanitl sandbox check
```

## Weiter

Die Dokumentation ist englisch: [`README.md`](README.md), Installation in
[`docs/guides/install.md`](docs/guides/install.md), die Sicherheitsargumentation in
[`docs/SECURITY.md`](docs/SECURITY.md). Die Oberfläche gibt es auf Deutsch und Englisch.
Sicherheitslücken bitte nicht als öffentliches Issue melden, sondern wie in
[`SECURITY.md`](SECURITY.md) beschrieben.
