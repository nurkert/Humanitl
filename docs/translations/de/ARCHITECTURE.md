---
source: ARCHITECTURE.md
source-sha256: 4bf2a8f58a3f54c37a160ddc20201a4cc2509ea5db697c52330925937df3d0d0
lang: de
---
# Humanitl — Architekturprinzipien

Applies to: Humanitl 0.1

> Zweck: Die Software soll von Anfang an in eine Form wachsen, die auch nach Jahren des Wachstums als Hobbyprojekt nicht historisch wuchert. Zugleich soll der Code so schlank wie möglich bleiben. Dieses Dokument legt fest, wie beides zusammengeht. Es ergänzt `BACKLOG.md` (Entscheidungen, Plan) und `backlog/CONVENTIONS.md` (kanonische Namen).

## 1. Die drei Sätze

1. **Ein Kern, der nichts von der Welt weiß.** Alles, was Humanitl *bedeutet* (Lebenszyklus eines Flows, Regeln, Funde, Diagnostics, Audit-Kette), lebt in Crates ohne IO, ohne async, ohne Protobuf. Diese Crates sind reine Funktionen über Werttypen und lassen sich tabellengetrieben testen.
2. **Ein Ereignisstrom als Rückgrat.** Jede Zustandsänderung eines Flows wird zu einem `FlowEvent`. Oberfläche, CLI, Recorder, Audit und später Plugins sind Abnehmer dieses Stroms. Niemand fragt den Proxy nach seinem Zustand; alle hören zu.
3. **Eine öffentliche Schnittstelle.** Das gRPC-Proto `humanitl.v1` ist der einzige Weg, mit dem Daemon zu sprechen. Oberfläche, CLI, Tests und Plugins sind gleichberechtigte Clients. Es gibt keine Hintertür für „mal eben schnell".

## 2. Schichten und Abhängigkeitsrichtung

```
                    ┌────────────────────────────────────────────┐
  Adapters (outer)  │ ipc (tonic)  sandbox/bwrap  proxy/hudsucker │  IO, async, third-party libraries
                    │ recorder/sqlite  catalog/files  cli/clap    │
                    │ audit-log/jsonl                             │
                    └───────────────────┬────────────────────────┘
                                        │ may point inwards
                    ┌───────────────────▼────────────────────────┐
  Application       │ session orchestration, hold queue,          │  async, but only via ports
                    │ policy decision (rule → verdict → hold)     │
                    └───────────────────┬────────────────────────┘
                                        │
                    ┌───────────────────▼────────────────────────┐
  Core (inner)      │ core-types  rules  findings  audit  config  │  no IO, no async, no proto
                    └────────────────────────────────────────────┘
```

Regeln:

- Abhängigkeiten zeigen nur nach innen. Der Kern importiert nichts aus der Anwendung oder den Adaptern. Die Anwendung kennt Adapter nur als Traits (Ports).
- Ein Port ist ein Trait im Kern oder in der Anwendung; ein Adapter ist seine Implementierung außerhalb. Im MVP hat jeder Port genau eine Implementierung. Ein zweiter Adapter (Docker, macOS, externes Plugin) berührt weder Kern noch Anwendung.
- Von der CI erzwungen: `cargo deny` für Lizenzen und Duplikate, ein Skript `tools/check-deps.sh`, das den Cargo-Graphen gegen die erlaubte Richtung prüft (aus `backlog/CONVENTIONS.md` 3.1), `#![deny(missing_docs)]` in allen Bibliotheks-Crates und, für die Kern-Crates in `[core]` von `tools/deps-allow.toml`, das Verbot der Abhängigkeiten in `[core].forbidden` zusammen mit einer Quellprüfung gegen die Muster in `[core].source` (Dateien, Laufzeit, eigene Threads und Prozesse, Sockets; HUM-155).

Ports (verbindliche Liste, vollständig für den MVP):

| Port | Definiert in | MVP-Adapter | Spätere Adapter |
|---|---|---|---|
| `SandboxBackend` | sandbox | bwrap | docker, seatbelt, microvm |
| `AgentAdapter` | core | opencode | aider, codex, claude-code |
| `Detector` | findings | regex/checksum | presidio-grpc, wasm |
| `FlowStore` | recorder | sqlite | postgres (team) |
| `AuditSink` | audit | jsonl-hashchain | remote-anchor |
| `CatalogSource` | catalog | bundled-yaml | user-yaml, team-url |
| `Resolver` | proxy | hickory (nach dem Erlauben) | doh |
| `Egress` | proxy | direct | http-proxy, socks5h (Tor) |
| `Bridge` (Shim) | sandbox | proxy-socket | cdp-socket (Browser) |
| `AskChannel` | application | grpc-ui, terminal, none | plugin |

## 3. Der Lebenszyklus eines Flows als Zustandsmaschine

```
Received ──analyze──▶ Analyzed ──evaluate──▶ Held ──decide──▶ Decided ──▶ Forwarded ──▶ Responded ──▶ Recorded
                          │                    │                 ▲
                          │ (rule: allow)      │ (timeout)       │
                          └────────────────────┴─────────────────┘
                          (rule: block) ──────────────────────────▶ Decided(Block) ──▶ Recorded
```

- Die Zustandsmaschine ist ein `enum` mit einer Methode `on(self, &FlowEvent) -> Result<Self, InvalidTransition>`. Einen anderen Weg, den Zustand zu ändern, gibt es nicht. Ungültige Übergänge sind Bugs und werden im Test vollständig aufgezählt.
- Der Proxy-Handler *treibt* die Zustandsmaschine an; er *besitzt* sie nicht. Er ruft `on` auf, veröffentlicht das Ereignis und wartet, wo nötig, auf die Entscheidung. So bleibt der Handler dünn und die Logik ohne Netzwerk testbar.
- Zeitüberschreitungen sind Ereignisse (`TimedOut`), keine Sonderpfade.

## 3b. Parität von Oberfläche und CLI

Jede Fähigkeit gibt es genau einmal, als RPC im Daemon. Oberfläche und CLI sind austauschbare, dünne Clients desselben Protos. Regeln: Ein neuer RPC bringt sein CLI-Unterkommando im selben Issue mit; das Gegenstück in der Oberfläche folgt spätestens im nächsten Sprint; `docs/reference/parity.md` wird erzeugt und in der CI geprüft (ADR-018). Fachlogik in `app/` oder `bin/humanitl` ist ein Architekturverstoß, auch wenn sie klein ist. Der übliche Anwendungsfall ist vollständig als Profil `default` beschrieben; Anpassung ändert Werte, nie Code.

## 4. Sparsamkeit im Code, konkret

Wachstum ist erlaubt, Wuchern nicht. Die Unterscheidung:

- **Kein Trait ohne zweiten Nutzer in Sicht.** Die Port-Liste oben ist abgeschlossen. Wer einen neuen Port will, schreibt zuerst ein ADR mit dem konkreten zweiten Adapter.
- **Keine Abstraktion über Fremdbibliotheken.** hudsucker, tonic, rusqlite werden direkt benutzt, in *einem* Crate. Ändert sich die Bibliothek, ändert sich dieses eine Crate.
- **Werttypen statt Strings.** `HostName`, `FlowId`, `DiagnosticCode` sind Typen. Ein `String` in einer Signatur ist ein Code-Smell, außer es ist wirklich freier Text.
- **Fehler sind Werte mit Bedeutung.** `Diagnostic` überall, wo ein Mensch die Ursache sehen könnte. `thiserror` pro Crate, kein `anyhow` in Bibliotheks-Crates, `anyhow` nur in `main`.
- **Eine Quelle für die Konfiguration.** Einstellungen sind Rust-Typen mit einem Schema. Oberfläche, CLI und Doku werden daraus erzeugt, nie parallel gepflegt.
- **Dünne Binaries.** `humanitld` und `humanitl` sind Verdrahtung, keine Logik. Enthält `main.rs` ein `if`, das eine fachliche Entscheidung trifft, steht es am falschen Ort.
- **Erzeugter Code ist kein Quellcode.** Proto-, riverpod- und freezed-Ausgaben werden von Git ignoriert und in der CI erzeugt. Drift bricht den Build.
- **Löschen ist ein Feature.** Jeder Sprint endet mit einem Blick auf `experimental`-Flags und ungenutzte Pfade. Was zwei Sprints lang niemand benutzt hat, fliegt raus.

## 5. Flutter-Seite, dieselben Regeln

- `core/domain` sind freezed-Werttypen, Spiegel der Rust-Kerntypen, ohne Flutter-Import.
- `core/ipc` ist eine Schnittstelle `DaemonClient` mit zwei Implementierungen: gRPC und Fake. Jeder Bildschirm wird gegen die Schnittstelle gebaut, jeder Widget-Test läuft gegen den Fake.
- Seit HUM-044 lebt dort auch der **Zustand der Verbindung** (`core/ipc/connection.dart`): `ConnectionStatus` und das Prädikat `linkLiveProvider`, das sagt, ob der Daemon gerade antwortet. Früher lebte er unter `features/shell`, aber die Warteschlange und der Ereignisstrom brauchen dieselbe Frage beantwortet und dürfen `features/shell` nicht importieren (Abschnitt 5, erzwungen durch `tools/check-deps.sh`). Eine Frage, die mehr als ein Feature stellt, gehört nach `core`. Sonst wird sie an jeder Stelle neu beantwortet, jedes Mal ein wenig anders, und genau das hatte zu fünf Befunden geführt.
- Aus demselben Grund lebt seit HUM-044 auch die **Uhr der Oberfläche** in `core` (`core/time/now.dart`): `nowProvider` ist der eine Taktgeber, aus dem jede Beschriftung, jeder Countdown und jede Laufzeit ihre Sekunde liest (`docs/UX.md` 7, „Zwei Uhren, nicht eine"). Sie lebte unter `features/intercept`, solange nur die Warteschlange sie brauchte; die Laufzeit der Sandbox liest jetzt dieselbe, damit beide im selben Moment stehen bleiben, wenn die Verbindung abreißt. Der alte Pfad exportiert die Datei weiterhin.
- Seit HUM-116 lebt auch die **Body-Ansicht** in `core` (`core/body/`): `BodyView` mit Baum, Formular, Roh- und Hex-Ansicht, einschließlich Entpacken und Parsen. Mit ihr zeigt die Warteschlange den Body der Anfrage, der Verlauf den von Anfrage und Antwort, und beide holen die Bytes über `flowBodyProvider` (`core/body/flow_body_provider.dart`), den einen Body-Provider (`backlog/CONVENTIONS.md` 3.9). Er lebt bei der Ansicht und nicht in `core/ipc`, weil er die Typen der Ansicht liefert (`RawBody`, `BodyKind`); `core/ipc` bleibt der Transport und importiert nichts aus `core/body`. Was die Bildschirme unterscheidet, geben sie hinein: die Header der Seite, zu der der Body gehört, und die Funde. Ihr Detail lesen sie weiter aus einem eigenen Provider.
- Features sind Verzeichnisse mit Bildschirm, Providern und Widgets. Ein Feature importiert kein anderes Feature, nur `core`. Übergreifende Widgets leben in `packages/ui`.
- Zustand fließt in eine Richtung: Ereignisstrom → Provider → Widget. Widgets rufen `DaemonClient` nur über Provider-Methoden auf (Mutationen), nie direkt.
- Das Widget-Vokabular lebt in `packages/ui` und baut auf `shadcn_flutter` auf (ADR-0009, Abschnitt „Revised on 2026-09-04 by the project owner"). Nur dieses Paket importiert die Bibliothek; ein Feature importiert `lib/core/ui/ui.dart`, nie ein fremdes Widget-Paket. `tools/check-deps.sh` prüft beides.

## 6. Tests als Wächter der Architektur

| Ebene | Was sie schützt | Wo |
|---|---|---|
| Unit, tabellengetrieben | Kernsemantik (Globs, Übergänge, Hash-Kette, Vorrang der Konfiguration) | `crates/*/src`, `#[cfg(test)]` |
| Integration im Prozess | Proxy-Handler gegen ein axum-Fake-Upstream, kein Unterprozess | `crates/proxy/tests` |
| Escape (privilegiert) | Die drei Garantien | `tests/escape`, eigener CI-Job |
| Widget + Golden | Zustände der Oberfläche gegen den Fake-Daemon | `app/test` |
| e2e | Ein Nutzerpfad pro Meilenstein | `tests/e2e`, xvfb |
| Fuzz (nächtlich) | Parser und Decoder | `daemon/fuzz` |
| Abhängigkeits-Lint | Abhängigkeitsrichtung | `tools/check-deps.sh` |

Regel: Ein Bug bekommt zuerst einen Test auf der niedrigsten Ebene, auf der er sich nachstellen lässt.

## 7. Was wir bewusst nicht tun

- Kein Event-Sourcing des Zustands im Speicher. Ereignisse sind Ausgabe, nicht Wahrheit. Wahrheit sind die Zustandsmaschine und SQLite.
- Kein Plugin-System im MVP. Nur die Ports. Plugins kommen als externe gRPC-Clients, sobald drei echte Bedarfe dokumentiert sind.
- Keine Mikroservices. Ein Daemon, eine Oberfläche, eine CLI. Trennung durch Crates, nicht durch Prozesse, mit einer Ausnahme: der Shim in der Sandbox, weil er ohne tokio und ohne Abhängigkeiten auskommen muss.
- Keine selbstgebaute Kryptographie. rustls, rcgen, sha2, hmac, aes-gcm aus RustCrypto, keyring.

## 8. Der Agent in der Sandbox (Bewusstsein und Rückmeldung)

Der Agent soll mit möglichst wenigen Tokens wissen, wo er ist, und auf Rückmeldungen reagieren können. Alles läuft über den einen vorhandenen Kanal, den Proxy-Socket. Kein neuer Kanal, keine neue Lücke in den Garantien.

1. **Briefing.** Der `AgentAdapter` legt eine globale Anweisungsdatei in die Sandbox (OpenCode: `~/.config/opencode/AGENTS.md`, Claude Code: `~/.claude/CLAUDE.md`), nie ins Projektverzeichnis. Ihr Inhalt ist eine mitgelieferte Vorlage von etwa 150 Tokens, in der Sprache des Nutzers. Kernaussagen: kein direkter Internetzugang; jede HTTP-Anfrage geht durch einen Proxy, ein Mensch entscheidet; Warten ist normal, nicht abbrechen; eine Antwort `403` mit `Blocked by Humanitl` ist endgültig, nicht wiederholen, stattdessen den Nutzer informieren und eine Alternative vorschlagen; Statusabfrage unter `http://humanitl.internal/`.
2. **Rückmeldung beim Blockieren.** Die Aktionsleiste hat ein optionales Notizfeld. Der Text landet im 403-Body unter `note:` und im Header `X-Humanitl-Note`. Der Agent sieht ihn im Tool-Ergebnis und kann reagieren („Nimm PyPI statt GitHub").
3. **Meta-Endpunkt.** Der Proxy bedient den virtuellen Host `humanitl.internal` selbst, ohne Upstream, nur `GET` und `POST` mit kleinen Textantworten: `/` kurzer Status und die aktuell gültigen Regeln, eine Zeile pro Regel; `/why/<flow-id>` der Grund für eine Entscheidung; `/ask` mit einem Freitext-Body, erscheint in der Oberfläche als Karte „Der Agent bittet um …" mit einem Regelvorschlag. Der Endpunkt ist überwiegend lesend; `/ask` legt nur eine Karte in der Oberfläche an, nie eine Regel.
4. **Sichtbarkeit im Terminal.** Wird eine Anfrage angehalten, schreibt der Daemon eine Zeile in das Terminal des Agenten (über den PTY-Strom, als Statuszeile, nicht in stdin). So sieht der Mensch beim Agenten, was gerade ansteht.

Was das nicht ist: Der Agent bekommt keine Möglichkeit, Regeln anzulegen, Entscheidungen zu beeinflussen oder den Nutzer zu umgehen. `/ask` ist eine Bitte, keine Aktion.
