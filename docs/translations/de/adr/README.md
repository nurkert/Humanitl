---
source: adr/README.md
source-sha256: 9655ce1ab3513436d2e8cfa729aa16ec9bd52850886a2ce24117afe32a5c2736
lang: de
---
# Architekturentscheidungen (ADRs)

Applies to: Humanitl 0.1

Jede Entscheidung, die die Architektur von Humanitl trägt, hat hier eine eigene Datei. Die
Kurzfassungen in `BACKLOG.md` Abschnitt 2 sind der Ursprung; die Dateien hier sind die
ausgearbeitete, verbindliche Fassung. Wer eine Entscheidung nachschlagen will, liest die
Datei. Sie ist so geschrieben, dass man sie ohne das Backlog versteht.

Das Format ist [MADR](https://adr.github.io/madr/) in einer schlanken Fassung. Jede Datei
hat dieselben sieben Überschriften: den Titel, dann `Context`, `Decision`, `Rationale`,
`Rejected alternatives`, `Consequences`, `Affected issues`. Unter dem Titel stehen
`Status:` und `Date:`. Eine Entscheidung, die von einem eigenen Issue überprüft wird, darf
einen achten Abschnitt tragen: den Nachtrag mit dem Ergebnis dieser Überprüfung, benannt
nach ihr (`Decision after Sprint N`). Er steht vor `Affected issues`, und die Abschnitte,
die er ersetzt, bekommen einen Hinweis, der auf ihn zeigt, statt überschrieben zu werden.
`check.sh` prüft die sieben Pflichtüberschriften und stört sich nicht an der achten.
Vorlage für neue Entscheidungen: [`0000-template.md`](0000-template.md).

Regeln:

- **Nummern werden nie umbenannt und nie wiederverwendet.** Die Reihenfolge im Backlog
  weicht an einer Stelle ab (`ADR-010` steht dort nach `ADR-018`); die Nummern folgen dem
  Backlog, nicht der Lesereihenfolge.
- **Zwei Schreibweisen derselben Nummer.** In den Dateien hier heißt eine Entscheidung
  `ADR-0007` mit vier Ziffern, passend zum Dateinamen. `BACKLOG.md`,
  `docs/ARCHITECTURE.md` und `backlog/CONVENTIONS.md` schreiben dieselbe Entscheidung mit
  drei Ziffern als `ADR-007`. Gemeint ist immer dieselbe Datei.
- Wird eine Entscheidung ersetzt, bleibt die alte Datei stehen und bekommt
  `Status: Superseded by ADR-XXXX`. Die neue Datei verweist zurück.
- Jede ADR nennt mindestens ein Issue aus `backlog/sprint-N.md`.
- `backlog/CONVENTIONS.md` Abschnitt 4 ist das laufende Protokoll der Korrekturen; landet
  dort eine Korrektur, wird die betroffene ADR auf den neuen Stand gebracht, statt die
  Abweichung stehen zu lassen.
- `docs/adr/check.sh` prüft die Nummerierung, die Vollständigkeit des Verzeichnisses und
  die sieben Überschriften. Es ist der ADR-Teil der Dokumentationsprüfung und so gebaut,
  dass `scripts/ci/lint-docs.sh` es aufruft (HUM-007), damit auch `make docs-lint` es
  ausführt.

## Verzeichnis

| Nr. | Titel | Status | Worum es geht |
|---|---|---|---|
| [0001](0001-rust-hudsucker.md) | Daemon in Rust auf hudsucker, nicht mitmproxy | Angenommen | Ein speichersicheres Binary statt eines Python-Bündels; hudsucker liefert CONNECT, TLS-Terminierung und den asynchronen Haltepunkt. |
| [0002](0002-bwrap-first.md) | Zuerst bubblewrap, Docker später als zweites Backend | Angenommen | `--unshare-all` ergibt einen leeren Netzwerk-Namespace; die Richtlinie ist eine lesbare Kommandozeile, die die Oberfläche anzeigt. |
| [0003](0003-grpc-uds.md) | gRPC über einen Unix-Domain-Socket als einzige Schnittstelle | Angenommen | Ein Vertrag (`humanitl.v1`) für Oberfläche, Kommandozeile, Tests und spätere Plugins, mit Streaming und Gegendruck. |
| [0004](0004-flow-state-machine.md) | Lebenszyklus einer Anfrage als Zustandsmaschine, Ereignisse abgeleitet | Angenommen | `FlowState` mit einer Übergangsmethode; jedes `FlowEvent` ist die Ausgabe eines Übergangs, der stattgefunden hat. |
| [0005](0005-buffer-request-body.md) | Den Body der Anfrage vollständig puffern, bevor der Mensch entscheidet | Angenommen | Wer nur Header freigibt, sieht den Teil nicht, in dem Daten ausgeleitet werden; Obergrenzen, Budget und ein einheitlicher Block-Body. |
| [0006](0006-dns-after-allow.md) | DNS-Auflösung erst nach dem Erlauben | Angenommen | Ein Hostname verrät 63 Bytes pro Label; er wird einmal nach `allow` aufgelöst, und die IP wird festgehalten. |
| [0007](0007-rule-model.md) | Regelmodell: geordnete Liste, der erste passende Eintrag gewinnt, Vorgabe `ask` | Angenommen | Label-Globs statt Teilzeichenketten, Punycode-Normalisierung, Session-Regeln zuerst, Domain-Fronting blockiert ohne Nachfrage. |
| [0008](0008-storage.md) | Speicherung: SQLite, Blob-Speicher, Audit-Kette, Pseudonym-Zuordnung | Angenommen | Metadaten in SQLite, große Bodies inhaltsadressiert, Audit als JSONL-Hash-Kette mit einer ehrlichen Aussage über ihre Grenzen. |
| [0009](0009-ui-stack.md) | Oberflächen-Stack: Flutter mit `shadcn_flutter` hinter `packages/ui` | Angenommen | Datenlastige Widgets aus spezialisierten Paketen, kein WebView; HUM-035 hat sich am 2026-09-04 dagegen entschieden, der Projektinhaber hat das am selben Tag rückgängig gemacht. |
| [0010](0010-packaging.md) | Auslieferung als `.deb` und AppImage, Flatpak später und nur für die Oberfläche | Angenommen | Ein Artefakt enthält alles; systemd-User-Unit mit einem Klick; Flatpak scheitert am `bwrap`-Start des Daemons. |
| [0011](0011-single-config-source.md) | Eine Konfigurationsquelle, drei Sichtbarkeitsstufen | Angenommen | Rust-Typ als einzige Quelle für Schema, CLI-Flags, Einstellungsbildschirm und Docs; `basic`/`advanced`/`expert`. |
| [0012](0012-diagnostics-as-type.md) | Geführte Zustände als Typ: `Diagnostic` statt einer Fehlerzeichenkette | Angenommen | Jeder Fehlerpfad liefert `code`, `why` und eine ausführbare `FixAction`; die Codes stehen in einem Register. |
| [0013](0013-cli-headless.md) | Die Kommandozeile als gleichwertiger Client und Betrieb ohne Oberfläche | Angenommen | Die Warteschlange der angehaltenen Anfragen lebt im Daemon; `--ask terminal` und `--ask none` als die beiden ehrlichen Modi ohne Oberfläche. |
| [0014](0014-agent-awareness.md) | Bewusstsein des Agenten und Rückmeldung über den einen vorhandenen Kanal | Angenommen | Briefing, Hinweis in der 403, Meta-Endpunkt `humanitl.internal`; kein neuer Kanal, keine neue Fähigkeit für den Agenten. |
| [0015](0015-ports-and-adapters.md) | Ports und Adapter mit maschinell erzwungener Abhängigkeitsrichtung | Angenommen | Kern ohne IO, geschlossene Liste der Ports, `tools/check-deps.sh` prüft den Cargo-Graphen statt eines Reviews. |
| [0016](0016-browser-cdp.md) | Browser für den Agenten über CDP, Zuschauen und Eingreifen in der Oberfläche | Angenommen | Nach dem MVP (M7); im MVP nur die Grundlage: Bridge-Liste im Shim und seccomp-Familien je Profil. |
| [0017](0017-egress-port.md) | Ein Egress-Port für direkte Verbindung, Upstream-Proxy und Tor | Angenommen | Genau eine Stelle, an der eine ausgehende Verbindung entsteht; die CI verbietet `TcpStream::connect` überall sonst. |
| [0018](0018-rpc-parity.md) | Parität: Jede Fähigkeit ist zuerst ein RPC, Oberfläche und Kommandozeile sind dünne Clients | Angenommen | Erzeugte Paritäts-Tabelle; ein RPC ohne Zeile für die Kommandozeile bricht den Build, eine Lücke in der Oberfläche ist nur eine Warnung. |
| [0019](0019-seatbelt-on-macos.md) | Seatbelt ist die Sandbox auf macOS, eine Linux-VM kommt später | Angenommen | `sandbox-exec` mit einem erzeugten SBPL-Profil je Session; die drei Garantien für macOS neu gefasst und mit demselben Shim gemessen. |

## Querverweise

Wo eine Entscheidung eine andere verfeinert oder korrigiert:

| Von | Nach | Beziehung |
|---|---|---|
| ADR-0006 | ADR-0001 | Die Auflösung nach dem Erlauben bestimmt, wie der Proxy Upstream-Verbindungen aufbaut. |
| ADR-0017 | ADR-0006 | Fasst den Verbindungsaufbau als Port und hält die IP aus ADR-0006 fest. |
| ADR-0007 | ADR-0006 | `allow_private: true` ist die benannte Ausnahme von der Blockade privater Zieladressen. |
| ADR-0005 | ADR-0004 | Legt die Statuscodes je `BlockReason` und den einheitlichen Block-Body fest. |
| ADR-0014 | ADR-0005 | Erweitert den Block-Body um `note:` und den Header `X-Humanitl-Note`. |
| ADR-0015 | ADR-0001, ADR-0002, ADR-0008, ADR-0017 | Legt fest, wo diese Adapter leben dürfen und dass ein zweiter Adapter den Kern nicht berührt. |
| ADR-0016 | ADR-0002 | Korrigiert die Aussage „`AF_UNIX` immer verboten": Im Profil `browser` ist es erlaubt, und dort trägt der Netzwerk-Namespace die Garantie. |
| ADR-0018 | ADR-0003, ADR-0013 | Macht die Gleichwertigkeit von Oberfläche und Kommandozeile aus ADR-0013 maschinell prüfbar. |
| ADR-0019 | ADR-0002, ADR-0015 | Der zweite Adapter von `SandboxBackend`, den ADR-0015 angekündigt hat; auf macOS tritt er an die Stelle von bwrap aus ADR-0002. |
| ADR-0011 | ADR-0007 | Regelt die beiden Speicherorte für Regeln (gespeichert und Session). |
| ADR-0012 | ADR-0010 | `FixAction::InstallService` ist der Mechanismus hinter der Installation mit einem Klick. |

Die größte Korrektur innerhalb einer Entscheidung steht in ADR-0009, und sie ging am
2026-09-04 zweimal hin und her. HUM-035 hat die Wahl der Komponentenbibliothek überarbeitet
und `shadcn_flutter` verworfen; der Projektinhaber hat das am selben Tag rückgängig
gemacht, weil die ursprüngliche Fassung der ADR seine Vorgabe war und ein Issue sie nicht
ungefragt kippen darf. Es gilt der Abschnitt „Revised on 2026-09-04 by the project owner":
`shadcn_flutter` wird übernommen, festgepinnt und ausschließlich in `app/packages/ui`
verwendet. Der Abschnitt „Decision after Sprint 2" bleibt als Geschichte stehen, weil seine
Messungen weiter gelten; die Abschnitte `Decision`, `Rationale`, `Rejected alternatives`
und `Consequences` tragen Hinweise, die auf den aktuellen Stand zeigen. Der Rest der
Entscheidung steht unverändert: Flutter, die Kapselung, kein WebView, ein Fenster mit
angedockten Bereichen.

Weitere Korrekturen innerhalb einer Entscheidung: ADR-0005 hält fest, dass
`Expect: 100-continue` **sofort** mit `100 Continue` beantwortet wird. Die frühere
Formulierung „erst nach der Entscheidung" hätte das Puffern des Body unmöglich gemacht.
ADR-0004 hält entsprechend fest, dass die Übergangsmethode das Ereignis **erzeugt** und
es nicht empfängt; die frühere Signatur `on(self, &FlowEvent)` aus
`docs/ARCHITECTURE.md` 3 ist überholt.

## Prüfung

```sh
docs/adr/check.sh              # checks this directory
docs/adr/check.sh --self-test  # proves that every check fires on a broken copy
```

Das Skript prüft, dass jede Markdown-Datei außer dem Verzeichnis dem Muster
`NNNN-kebab-title.md` folgt, dass die Nummern lückenlos und eindeutig sind, dass jede
Datei im Verzeichnis steht und jeder Eintrag im Verzeichnis eine Datei hat, dass Status
und Datum vorhanden sind, dass jede Datei die sieben Überschriften trägt und dass jede
mindestens ein `HUM-`-Issue nennt. `--self-test` legt dreizehn Kopien mit genau einem
Fehler je Kopie an (falscher Dateiname, Lücke, Dublette, fehlende Überschrift, Status in
Kleinbuchstaben, …) und erwartet für jede die passende Meldung.
