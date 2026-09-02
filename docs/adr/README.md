# Architektur-Entscheidungen (ADRs)

Jede Entscheidung, die die Architektur von Humanitl trägt, hat hier eine eigene
Datei. Die Kurzfassungen in `BACKLOG.md` Abschnitt 2 sind der Ursprung; die
Dateien hier sind die ausgearbeitete, verbindliche Fassung. Wer eine Entscheidung
nachschlagen will, liest die Datei — sie ist so geschrieben, dass sie ohne den
Backlog verständlich ist.

Format ist [MADR](https://adr.github.io/madr/) in einer schlanken Fassung. Jede
Datei hat dieselben sieben Überschriften: Titel, dann `Kontext`, `Entscheidung`,
`Begründung`, `Verworfene Alternativen`, `Konsequenzen`, `Betroffene Issues`.
Darunter stehen `Status:` und `Datum:`. Vorlage für neue Entscheidungen:
[`0000-template.md`](0000-template.md).

Regeln:

- **Nummern werden nie umbenannt und nie wiederverwendet.** Die Reihenfolge im
  Backlog weicht an einer Stelle ab (`ADR-010` steht dort hinter `ADR-018`); die
  Nummern folgen dem Backlog, nicht der Lesereihenfolge.
- **Zwei Schreibweisen derselben Nummer.** In den Dateien hier heißt eine
  Entscheidung vierstellig `ADR-0007`, passend zum Dateinamen. `BACKLOG.md`,
  `docs/ARCHITECTURE.md` und `backlog/CONVENTIONS.md` schreiben dieselbe
  Entscheidung dreistellig als `ADR-007`. Gemeint ist immer dieselbe Datei.
- Wird eine Entscheidung ersetzt, bleibt die alte Datei stehen und bekommt
  `Status: Superseded by ADR-XXXX`. Die neue Datei verweist zurück.
- Jede ADR nennt mindestens ein Issue aus `backlog/sprint-N.md`.
- `backlog/CONVENTIONS.md` Abschnitt 4 ist das laufende Protokoll der
  Korrekturen; landet dort eine Korrektur, wird die betroffene ADR nachgezogen,
  statt dass die Abweichung stehen bleibt.
- `docs/adr/check.sh` prüft Nummernvergabe, Vollständigkeit des Index und die
  sieben Überschriften. Es ist der ADR-Teil der Dokumentationsprüfung und dafür
  gebaut, aus `scripts/ci/lint-docs.sh` (HUM-007) aufgerufen zu werden, damit
  `make docs-lint` es mit ausführt.

## Index

| Nr. | Titel | Status | Worum es geht |
|---|---|---|---|
| [0001](0001-rust-hudsucker.md) | Daemon in Rust auf hudsucker, nicht mitmproxy | Accepted | Ein speichersicheres Binary statt eines Python-Bundles; hudsucker liefert CONNECT, TLS-Terminierung und den async Haltepunkt. |
| [0002](0002-bwrap-first.md) | bubblewrap zuerst, Docker später als zweites Backend | Accepted | `--unshare-all` gibt ein leeres Netzwerk-Namespace; die Policy ist eine lesbare Kommandozeile, die das UI zeigt. |
| [0003](0003-grpc-uds.md) | gRPC über Unix Domain Socket als einzige Schnittstelle | Accepted | Ein Vertrag (`humanitl.v1`) für UI, CLI, Tests und spätere Plugins, mit Streaming und Backpressure. |
| [0004](0004-flow-state-machine.md) | Request-Lebenszyklus als Zustandsautomat, Events abgeleitet | Accepted | `FlowState` mit einer Übergangsmethode; jedes `FlowEvent` ist Ausgabe eines stattgefundenen Übergangs. |
| [0005](0005-buffer-request-body.md) | Request-Body vollständig puffern, bevor der Mensch entscheidet | Accepted | Wer nur Header freigibt, sieht den Teil nicht, in dem exfiltriert wird; Caps, Budget und einheitlicher Block-Body. |
| [0006](0006-dns-after-allow.md) | DNS-Auflösung erst nach der Freigabe | Accepted | Ein Hostname leakt 63 Bytes pro Label; aufgelöst wird einmal nach `allow`, die IP wird gepinnt. |
| [0007](0007-rule-model.md) | Regel-Modell: geordnete Liste, first match wins, Default `ask` | Accepted | Label-Globs statt Substrings, Punycode-Normalisierung, Session-Regeln zuerst, Domain Fronting blockt ohne Nachfrage. |
| [0008](0008-storage.md) | Speicherung: SQLite, Blob-Store, Audit-Kette, Pseudonym-Mapping | Accepted | Metadaten in SQLite, große Bodies content-addressed, Audit als JSONL-Hash-Kette mit ehrlicher Aussage über ihre Grenzen. |
| [0009](0009-ui-stack.md) | UI-Stack: Flutter mit shadcn_flutter, gekapselt hinter `packages/ui` | Accepted | Chrome aus shadcn, datenlastige Widgets aus Spezialpaketen, kein WebView; Bestätigung Ende Sprint 2. |
| [0010](0010-packaging.md) | Auslieferung als `.deb` und AppImage, Flatpak später und nur für die UI | Accepted | Ein Artefakt enthält alles; systemd user unit per Klick; Flatpak scheitert am `bwrap`-Start des Daemons. |
| [0011](0011-single-config-source.md) | Eine Konfigurationsquelle, drei Sichtbarkeitsstufen | Accepted | Rust-Typ als einzige Quelle für Schema, CLI-Flags, Einstellungsbildschirm und Doku; `basic`/`advanced`/`expert`. |
| [0012](0012-diagnostics-as-type.md) | Geführte Zustände als Typ: `Diagnostic` statt Fehlerstring | Accepted | Jeder Fehlerpfad liefert `code`, `why` und eine ausführbare `FixAction`; Codes stehen in einem Register. |
| [0013](0013-cli-headless.md) | CLI als gleichwertiger Client und Headless-Betrieb | Accepted | Hold-Queue lebt im Daemon; `--ask terminal` und `--ask none` als die zwei ehrlichen Modi ohne Oberfläche. |
| [0014](0014-agent-awareness.md) | Agent-Bewusstsein und Feedback über den einen vorhandenen Kanal | Accepted | Briefing, Notiz im 403, Meta-Endpunkt `humanitl.internal` — kein neuer Kanal, keine neue Fähigkeit für den Agenten. |
| [0015](0015-ports-and-adapters.md) | Ports-and-Adapters mit maschinell erzwungener Abhängigkeitsrichtung | Accepted | Kern ohne IO, abgeschlossene Port-Liste, `tools/check-deps.sh` prüft den Cargo-Graphen statt eines Reviews. |
| [0016](0016-browser-cdp.md) | Browser für den Agenten über CDP, Zuschauen und Eingreifen im UI | Accepted | Post-MVP (M7); im MVP nur die Vorarbeiten: Bridge-Liste im Shim und seccomp-Familien pro Profil. |
| [0017](0017-egress-port.md) | Ein Egress-Port für Direktverbindung, Upstream-Proxy und Tor | Accepted | Genau ein Ort, an dem eine Verbindung nach außen entsteht; CI verbietet `TcpStream::connect` daneben. |
| [0018](0018-rpc-parity.md) | Parität: Jede Fähigkeit ist zuerst ein RPC, UI und CLI sind dünne Clients | Accepted | Generierte Paritäts-Tabelle; ein RPC ohne CLI-Zeile bricht den Build, eine UI-Lücke ist nur eine Warnung. |

## Querbezüge

Wo eine Entscheidung eine andere verfeinert oder korrigiert:

| Von | Nach | Verhältnis |
|---|---|---|
| ADR-0006 | ADR-0001 | Die Auflösung nach der Freigabe bestimmt, wie der Proxy Upstream-Verbindungen aufbaut. |
| ADR-0017 | ADR-0006 | Formalisiert den Verbindungsaufbau als Port und pinnt die IP aus ADR-0006. |
| ADR-0007 | ADR-0006 | `allow_private: true` ist die benannte Ausnahme von der Sperre privater Zieladressen. |
| ADR-0005 | ADR-0004 | Legt die Statuscodes je `BlockReason` und den einheitlichen Block-Body fest. |
| ADR-0014 | ADR-0005 | Erweitert den Block-Body um `note:` und den Header `X-Humanitl-Note`. |
| ADR-0015 | ADR-0001, ADR-0002, ADR-0008, ADR-0017 | Legt fest, wo diese Adapter liegen dürfen und dass ein zweiter Adapter den Kern nicht berührt. |
| ADR-0016 | ADR-0002 | Korrigiert die Aussage „`AF_UNIX` immer verboten": im Profil `browser` ist es erlaubt, die Garantie trägt dort das Netzwerk-Namespace. |
| ADR-0018 | ADR-0003, ADR-0013 | Macht die Gleichwertigkeit von UI und CLI aus ADR-0013 maschinell prüfbar. |
| ADR-0011 | ADR-0007 | Regelt die zwei Ablageorte für Regeln (gespeichert und Session). |
| ADR-0012 | ADR-0010 | `FixAction::InstallService` ist der Mechanismus hinter der Ein-Klick-Installation. |

Eine Korrektur innerhalb einer Entscheidung: ADR-0005 hält fest, dass
`Expect: 100-continue` **sofort** mit `100 Continue` beantwortet wird. Die
frühere Formulierung „erst nach der Entscheidung" hätte das Puffern des Bodys
unmöglich gemacht. ADR-0004 hält entsprechend fest, dass die Übergangsmethode das
Ereignis **erzeugt** und nicht entgegennimmt — die frühere Signatur
`on(self, &FlowEvent)` aus `docs/ARCHITECTURE.md` 3 ist überholt.

## Prüfung

```sh
docs/adr/check.sh              # prüft dieses Verzeichnis
docs/adr/check.sh --self-test  # beweist, dass jede Prüfung auf einer kaputten Kopie anschlägt
```

Das Skript prüft, dass jede Markdown-Datei außer dem Index dem Muster
`NNNN-kebab-titel.md` folgt, dass die Nummern lückenlos und eindeutig sind, dass
jede Datei im Index steht und jeder Index-Eintrag eine Datei hat, dass Status
und Datum vorhanden sind, dass jede Datei die sieben Überschriften trägt und
dass jede mindestens ein `HUM-`-Issue nennt. `--self-test` legt dreizehn
Kopien mit je genau einem Fehler an (falscher Dateiname, Lücke, Duplikat,
fehlende Überschrift, kleingeschriebener Status, …) und erwartet für jede die
passende Meldung.
