---
source: README.md
source-sha256: 78c019b98a1c9248eb485602769118bda547a3f8853030ba53c86b328a3d5a9c
lang: de
---
# Dokumentation

Applies to: Humanitl 0.1

Wo was steht. Seiten mit dem Vermerk *erzeugt* entstehen über `cargo xtask docs` aus dem
Code; die CI schlägt fehl, wenn sie veraltet sind.

## Humanitl benutzen

| Seite | Was sie beantwortet |
|---|---|
| [`guides/install.md`](guides/install.md) | Voraussetzungen, `.deb`, AppImage, der Nutzerdienst, Prüfungen, Entfernen |
| [`guides/first-session.md`](guides/first-session.md) | vom Bildschirm „Einrichten“ bis zur ersten angehaltenen Anfrage, einer Regel und der Prüfung der Audit-Kette |
| [`guides/agent-profiles.md`](guides/agent-profiles.md) | Session-Profile, der OpenCode-Adapter, einen Adapter schreiben, Hosts, die nach Hause telefonieren |

## Referenz

| Seite | Was sie beantwortet |
|---|---|
| [`reference/rules.md`](reference/rules.md) | jedes Feld einer Regel, Semantik des Abgleichs, Reihenfolge der Auswertung, Rezepte, mitgelieferte Regeln |
| [`reference/cli.md`](reference/cli.md) | jeder Befehl und jedes Flag (*erzeugt*) |
| [`CONFIG.md`](CONFIG.md) | jeder Schlüssel der Konfiguration mit Typ, Vorgabe, Stufe und Projekt-Geltung (*erzeugt*, deutsch) |
| [`DIAGNOSTICS.md`](DIAGNOSTICS.md) | jeder Diagnose-Code mit Auslöser und Abhilfe (*erzeugt*, deutsch) |
| [`reference/limits.md`](reference/limits.md) | die Ressourcengrenzen und `stream: true` |
| [`reference/parity.md`](reference/parity.md) | welchen RPC jeder CLI-Befehl und jeder Bildschirm nutzt (*erzeugt*) |
| [`cli.md`](cli.md) | was `humanitl run`, `doctor`, `config`, `audit` und `daemon` Schritt für Schritt tun |
| [`PROTOCOL.md`](PROTOCOL.md) | der gRPC-Vertrag |
| [`GLOSSARY.md`](GLOSSARY.md) | Begriffe auf Deutsch und Englisch |

## Sicherheit

| Seite | Was sie beantwortet |
|---|---|
| [`SECURITY.md`](SECURITY.md) | die Aussage, die drei Garantien, Seitenkanäle, Proxy, Regeln, Audit, Meldung |
| [`THREAT-MODEL.md`](THREAT-MODEL.md) | Werte, Angreifer, Vertrauensgrenzen, Angriffsfläche, Restrisiken |

## Humanitl bauen

| Seite | Was sie beantwortet |
|---|---|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | Schichten, Crates und die Abhängigkeitsregeln, die die CI durchsetzt |
| [`DESIGN.md`](DESIGN.md) | das Designsystem „Airlock“: Tokens, Komponenten, Bewegung |
| [`UX.md`](UX.md) | Bildschirme und Abläufe |
| [`INSTALL.md`](INSTALL.md) | Notizen für Maintainer: APT-Repository, Härtung der Unit, Paketbau, Testmatrix |
| [`adr/README.md`](adr/README.md) | Architekturentscheidungen |
