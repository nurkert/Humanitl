# Humanitl — Architektur-Leitbild

> Zweck: Die Software soll von Anfang an in eine Form hineinwachsen, die auch bei jahrelangem Hobby-Wachstum nicht historisch wuchert. Gleichzeitig soll der Code so schmal wie möglich bleiben. Dieses Dokument legt fest, wie beides zusammengeht. Es ergänzt `BACKLOG.md` (Entscheidungen, Plan) und `backlog/CONVENTIONS.md` (kanonische Namen).

## 1. Die drei Sätze

1. **Ein Kern, der nichts von der Welt weiß.** Alles, was Humanitl *bedeutet* (Flow-Lebenszyklus, Regeln, Findings, Diagnostics, Audit-Kette), lebt in Crates ohne IO, ohne async, ohne Protobuf. Diese Crates sind pure Funktionen über Werttypen und lassen sich tabellengetrieben testen.
2. **Ein Ereignisstrom als Rückgrat.** Jede Zustandsänderung eines Flows wird zu einem `FlowEvent`. UI, CLI, Recorder, Audit und spätere Plugins sind Konsumenten dieses Stroms. Niemand fragt den Proxy nach seinem Zustand, alle hören zu.
3. **Eine öffentliche Schnittstelle.** Die gRPC-Proto `humanitl.v1` ist die einzige Art, mit dem Daemon zu reden. UI, CLI, Tests und Plugins sind gleichberechtigte Clients. Es gibt keinen Hintereingang für „nur mal schnell".

## 2. Schichten und Abhängigkeitsrichtung

```
                    ┌────────────────────────────────────────────┐
  Adapter (außen)   │ ipc (tonic)  sandbox/bwrap  proxy/hudsucker │  IO, async, Fremdbibliotheken
                    │ recorder/sqlite  catalog/files  cli/clap    │
                    └───────────────────┬────────────────────────┘
                                        │ darf nach innen
                    ┌───────────────────▼────────────────────────┐
  Anwendung         │ Session-Orchestrierung, Hold-Queue,         │  async, aber nur über Ports
                    │ Policy-Entscheidung (Regel → Verdict → Hold)│
                    └───────────────────┬────────────────────────┘
                                        │
                    ┌───────────────────▼────────────────────────┐
  Kern (innen)      │ core-types  rules  findings  audit  config  │  kein IO, kein async, kein Proto
                    └────────────────────────────────────────────┘
```

Regeln:

- Abhängigkeiten zeigen nur nach innen. Der Kern importiert nichts aus Anwendung oder Adaptern. Die Anwendung kennt Adapter nur als Traits (Ports).
- Ein Port ist ein Trait im Kern oder in der Anwendung; ein Adapter ist seine Implementierung außen. Im MVP hat jeder Port genau eine Implementierung. Ein zweiter Adapter (Docker, macOS, externes Plugin) berührt Kern und Anwendung nicht.
- Erzwungen durch CI: `cargo deny` für Lizenzen und Duplikate, ein Skript `tools/check-deps.sh`, das den Cargo-Graphen gegen die erlaubte Richtung prüft (aus `backlog/CONVENTIONS.md` 3.1), und `#![deny(missing_docs)]` in allen Bibliotheks-Crates.

Ports (verbindliche Liste, vollständig für den MVP):

| Port | Wo definiert | MVP-Adapter | Spätere Adapter |
|---|---|---|---|
| `SandboxBackend` | sandbox | bwrap | docker, seatbelt, microvm |
| `AgentAdapter` | core | opencode | aider, codex, claude-code |
| `Detector` | findings | regex/checksum | presidio-grpc, wasm |
| `FlowStore` | recorder | sqlite | postgres (Team) |
| `AuditSink` | audit | jsonl-hashchain | remote-anchor |
| `CatalogSource` | catalog | bundled-yaml | user-yaml, team-url |
| `Resolver` | proxy | hickory (nach Allow) | doh |
| `Egress` | proxy | direct | http-proxy, socks5h (Tor) |
| `Bridge` (Shim) | sandbox | proxy-socket | cdp-socket (Browser) |
| `AskChannel` | Anwendung | grpc-ui, terminal, none | plugin |

## 3. Der Flow-Lebenszyklus als Zustandsautomat

```
Received ──analyze──▶ Analyzed ──evaluate──▶ Held ──decide──▶ Decided ──▶ Forwarded ──▶ Responded ──▶ Recorded
                          │                    │                 ▲
                          │ (Regel: allow)     │ (Timeout)       │
                          └────────────────────┴─────────────────┘
                          (Regel: block) ─────────────────────────▶ Decided(Block) ──▶ Recorded
```

- Der Automat ist ein `enum` mit einer Methode `on(self, &FlowEvent) -> Result<Self, InvalidTransition>`. Es gibt keinen anderen Weg, den Zustand zu ändern. Ungültige Übergänge sind Bugs und werden im Test vollständig aufgezählt.
- Der Proxy-Handler *treibt* den Automaten, er *besitzt* ihn nicht. Er ruft `on`, veröffentlicht das Event, und wartet gegebenenfalls auf die Entscheidung. Damit ist der Handler dünn und die Logik testbar ohne Netzwerk.
- Timeouts sind Events (`TimedOut`), keine Sonderpfade.

## 3b. Parität von UI und CLI

Jede Fähigkeit existiert genau einmal, als RPC im Daemon. UI und CLI sind austauschbare, dünne Clients derselben Proto. Regeln: ein neuer RPC bringt sein CLI-Subkommando im selben Issue mit; die UI-Entsprechung folgt spätestens im nächsten Sprint; `docs/reference/parity.md` wird generiert und in CI geprüft (ADR-018). Fachlogik in `app/` oder `bin/humanitl` ist ein Architekturverstoß, auch wenn sie klein ist. Der gängige Anwendungsfall ist als Profil `default` vollständig beschrieben; Anpassung ändert Werte, nie Code.

## 4. Code-Sparsamkeit, konkret

Wachstum ist erlaubt, Wucherung nicht. Die Unterscheidung:

- **Kein Trait ohne zweiten Nutzer in Sicht.** Die Port-Liste oben ist abgeschlossen. Wer einen neuen Port will, schreibt zuerst einen ADR mit dem konkreten zweiten Adapter.
- **Keine Abstraktion über Fremdbibliotheken.** hudsucker, tonic, rusqlite werden direkt benutzt, in *einer* Crate. Wenn die Bibliothek wechselt, wechselt diese eine Crate.
- **Werttypen statt Strings.** `HostName`, `FlowId`, `DiagnosticCode` sind Typen. Ein `String` an einer Signatur ist ein Code-Smell, außer er ist wirklich Freitext.
- **Fehler sind Werte mit Bedeutung.** `Diagnostic` überall, wo ein Mensch die Ursache sehen könnte. `thiserror` pro Crate, kein `anyhow` in Bibliotheks-Crates, `anyhow` nur in `main`.
- **Eine Konfigurationsquelle.** Settings sind Rust-Typen mit Schema. UI, CLI und Doku werden daraus erzeugt, nie parallel gepflegt.
- **Dünne Binaries.** `humanitld` und `humanitl` sind Verdrahtung, keine Logik. Wenn in `main.rs` ein `if` steht, das eine Fachentscheidung trifft, ist es am falschen Ort.
- **Generierter Code ist kein Quellcode.** Proto-Ausgaben, riverpod- und freezed-Ausgaben sind gitignored und werden in CI erzeugt. Drift bricht den Build.
- **Löschen ist eine Feature.** Jeder Sprint endet mit einem Blick auf `experimental`-Flags und ungenutzte Pfade. Was seit zwei Sprints niemand nutzt, fliegt.

## 5. Flutter-Seite, dieselben Regeln

- `core/domain` sind freezed-Werttypen, Spiegel der Rust-Kerntypen, ohne Flutter-Import.
- `core/ipc` ist ein Interface `DaemonClient` mit zwei Implementierungen: gRPC und Fake. Jeder Screen ist gegen das Interface gebaut, jeder Widget-Test läuft gegen Fake.
- Features sind Verzeichnisse mit Screen, Providern, Widgets. Ein Feature importiert kein anderes Feature, nur `core`. Übergreifende Widgets liegen in `packages/ui`.
- Zustand fließt in eine Richtung: Event-Stream → Provider → Widget. Widgets rufen `DaemonClient` nur über Provider-Methoden auf (Mutationen), nie direkt.
- Das Widget-Vokabular steht in `packages/ui` und baut auf `shadcn_flutter` (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer"). Nur dieses Paket importiert die Bibliothek; ein Feature importiert `lib/core/ui/ui.dart`, nie ein fremdes Widget-Paket. `tools/check-deps.sh` prüft beides.

## 6. Tests als Architektur-Wächter

| Ebene | Was sie schützt | Wo |
|---|---|---|
| Unit, tabellengetrieben | Kern-Semantik (Globs, Übergänge, Hash-Kette, Config-Präzedenz) | `crates/*/src`, `#[cfg(test)]` |
| Integration in-process | Proxy-Handler gegen axum-Fake-Upstream, kein Subprozess | `crates/proxy/tests` |
| Escape (privilegiert) | Die drei Garantien | `tests/escape`, eigener CI-Job |
| Widget + Golden | UI-Zustände gegen Fake-Daemon | `app/test` |
| e2e | Ein Nutzerweg pro Milestone | `tests/e2e`, xvfb |
| Fuzz (nightly) | Parser und Decoder | `daemon/fuzz` |
| Dependency-Lint | Abhängigkeitsrichtung | `tools/check-deps.sh` |

Regel: Ein Bug bekommt zuerst einen Test auf der niedrigsten Ebene, auf der er reproduzierbar ist.

## 7. Was wir bewusst nicht tun

- Kein Event Sourcing des In-Memory-Zustands. Events sind Ausgabe, nicht Wahrheit. Wahrheit ist der Automat plus SQLite.
- Kein Plugin-System im MVP. Nur die Ports. Plugins kommen als externe gRPC-Clients, wenn drei echte Bedarfe dokumentiert sind.
- Keine Mikroservices. Ein Daemon, ein UI, ein CLI. Trennung durch Crates, nicht durch Prozesse, mit einer Ausnahme: der Shim in der Sandbox, weil er ohne tokio und ohne Abhängigkeiten sein muss.
- Keine eigene Kryptographie. rustls, rcgen, sha2, hmac, aes-gcm aus RustCrypto, keyring.

## 8. Der Agent in der Sandbox (Bewusstsein und Feedback)

Der Agent soll mit möglichst wenigen Token wissen, wo er ist, und auf Rückmeldungen reagieren können. Alles läuft über den einen vorhandenen Kanal, den Proxy-Socket. Kein neuer Kanal, keine neue Garantie-Lücke.

1. **Briefing.** Der `AgentAdapter` legt in der Sandbox eine globale Instruktionsdatei an (OpenCode: `~/.config/opencode/AGENTS.md`, Claude Code: `~/.claude/CLAUDE.md`), nie im Projektverzeichnis. Inhalt ist eine gebündelte Vorlage von etwa 150 Token, in der Sprache des Nutzers. Kernaussagen: kein direkter Internetzugang; jede HTTP-Anfrage geht durch einen Proxy, ein Mensch entscheidet; Warten ist normal, nicht abbrechen; eine Antwort `403` mit `Blocked by Humanitl` ist endgültig, nicht wiederholen, stattdessen den Nutzer informieren und eine Alternative vorschlagen; Statusabfrage unter `http://humanitl.internal/`.
2. **Feedback beim Blocken.** Die Aktionsleiste hat ein optionales Notizfeld. Der Text landet im 403-Body unter `note:` und im Header `X-Humanitl-Note`. Der Agent sieht ihn im Tool-Ergebnis und kann reagieren („Nutze PyPI statt GitHub").
3. **Meta-Endpoint.** Der Proxy bedient den virtuellen Host `humanitl.internal` selbst, ohne Upstream, nur `GET` und `POST` mit kleinen Text-Antworten: `/` Kurzstatus und die aktuell gültigen Regeln in einer Zeile pro Regel; `/why/<flow-id>` Begründung einer Entscheidung; `/ask` mit Freitext-Body, erscheint im UI als Karte „Der Agent bittet um …" mit Regel-Vorschlag. Der Endpoint ist read-mostly, `/ask` erzeugt nur eine UI-Karte, nie eine Regel.
4. **Sichtbarkeit im Terminal.** Wenn ein Request gehalten wird, schreibt der Daemon eine Zeile in das Terminal des Agenten (über den PTY-Stream, als Statuszeile, nicht in stdin). So sieht der Mensch am Agenten, was gerade hängt.

Was das nicht ist: Der Agent bekommt keine Möglichkeit, Regeln anzulegen, Entscheidungen zu beeinflussen oder den Nutzer zu umgehen. `/ask` ist eine Bitte, keine Aktion.
