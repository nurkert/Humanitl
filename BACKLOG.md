# Humanitl — Backlog: von Null zum MVP

> Stand: 2026-09-02. Ergebnis aus Recherche (Isolation, Proxy-Engines, Flutter-Stack, Agent-Runtime, UX-Referenzen) und einer internen Review-Runde mit sechs Perspektiven (Security-Skeptiker, Pragmatischer Staff Engineer, Flutter/UX-Lead, Visual Designer, Usability-Kritiker, plus die Recherche-Synthese). Quellen am Ende.

---

## 0. Kurzfassung

**Was Humanitl ist.** Eine Linux-Desktop-Anwendung (Flutter), die einen KI-Coding-Agenten (zuerst OpenCode) in einer Sandbox startet, in der der Agent nur mit einem lokalen LLM im LAN und mit einem moderierenden Proxy sprechen kann. Jede andere Anfrage ins Netz wird angehalten, im UI angezeigt (Header, Parameter, Body, Domain-Kontext), kann editiert oder pseudonymisiert werden, und geht erst nach Freigabe raus. Regeln (allow / block / ask) reduzieren den Lärm. Alles wird aufgezeichnet.

**Der Sicherheitssatz.** Die Sandbox hat kein Netzwerk-Interface, es gibt keinen Weg nach draußen, und der Kernel verweigert jede Socket-Familie außer Loopback-TCP. Ihre einzigen Ein- und Ausgänge sind das Projektverzeichnis `/work` und genau ein Unix-Socket zum Proxy. Jedes Byte auf diesem Socket ist entweder eine von dir freigegebene HTTP-Anfrage oder ein Aufruf an den deklarierten LLM-Host, und beides wird aufgezeichnet.

**Getroffene Grundentscheidungen.**

| Thema | Entscheidung |
|---|---|
| UI | Flutter 3.47.2 (Pin `app/.fvmrc`, folgt dem neuesten stabilen Stand), Widget-Vokabular in `packages/ui` auf `shadcn_flutter`, riverpod 3, feature-first |
| Daemon | Rust, ein statisches Binary `humanitld`, systemd user service |
| Proxy-Engine | hudsucker (hyper 1 + rustls + rcgen), MITM mit eigener CA pro Installation |
| Sandbox MVP | bubblewrap `--unshare-all` + Unix-Socket-Bridge + seccomp (Muster von Claude Code `sandbox-runtime` und OpenAI Codex CLI) |
| IPC | gRPC über Unix Domain Socket, Proto ist der versionierte Vertrag |
| Default-Policy | `ask`. Timeout konfigurierbar, bei Ablauf `block`, nie `allow` |
| MVP-Scope Moderation | Nur Requests werden angehalten. Responses werden aufgezeichnet |
| Sprache | Englisch + Deutsch ab Tag eins (ARB) |
| Lizenz | GPL-3.0 (bereits im Repo) |

**Aufwandsschätzung MVP.** Solo, stark KI-unterstützt: 50 Arbeitstage netto als Untergrenze (38 aus der Review plus CLI, Profile, Settings-Screen, Diagnostics, Agent-Briefing, Meta-Endpoint, Doctor, LLM-Discovery, Ein-Klick-Installation, Pseudonym-Rücktausch), realistisch 50 bis 80 Tage, weil neun Teilsysteme in zwei Sprachen mit Low-Level-Anteil (Namespaces, seccomp, PTY, TLS-MITM) zusammenkommen. Geplant als 6 Sprints à 2 Wochen; wenn ein Sprint sein Demo-Skript nicht schafft, rutscht er, nicht der Inhalt. Sprint 5 ist Puffer.

---

## 1. Vision und Prinzipien

### 1.1 Problem

Lokale LLMs werden stark genug für echte Arbeit, wissen aber immer weniger selbst und müssen nachschlagen. Ein Agent mit Internetzugriff kann sensible Daten aus dem Projekt in eine Anfrage packen und nach außen schicken, sei es durch Prompt Injection, ein bösartiges Paket oder ein manipuliertes Modell. Heute bleibt nur: kein Internet. Humanitl macht Internet möglich, aber unter menschlicher Kontrolle, nachvollziehbar und aufgezeichnet.

### 1.2 Zielnutzer

Professionelle (Designer, Entwickler, Berater), die Kundendaten verarbeiten und DSGVO-Pflichten haben. Nicht zwingend Security-Experten. Sitzen live daneben, sind aber auch mal fünf Minuten weg.

### 1.3 Prinzipien

1. **Physik statt Vertrauen.** Der Agent kann nicht raus, weil kein Weg existiert, nicht weil er sich an `HTTP_PROXY` hält. Tools, die den Proxy ignorieren, scheitern (fail-closed).
2. **Etablierte Bausteine.** bubblewrap, seccomp, hyper/rustls, SQLite, systemd. Nichts Eigenes, wo Bewährtes existiert. Die Sicherheitsargumentation muss in drei Sätzen erklärbar und per Klick prüfbar sein.
3. **Ehrlich über Grenzen.** Der LLM-Kanal und `/work` sind deklarierte Seitenkanäle, werden im UI gezeigt, nicht versteckt.
4. **Schön und intuitiv.** Ruhiger Kontrollraum, nicht Hacker-Tool. Farbe bedeutet Zustand, nie Dekoration. Entscheidungen inline, nie modal.
5. **Saubere Codebasis.** Klare Modulgrenzen, typisierte IDs, Zustandsautomat für den Request-Lebenszyklus, Tests auf jeder Ebene, Erweiterungspunkte von Anfang an als Traits/Interfaces.
6. **Erweiterbar, aber später.** Plugins werden nicht im MVP gebaut, die Architektur lässt sie aber ohne Umbau zu (siehe Abschnitt 6).
7. **Das Tool leitet an.** Jeder Zustand, der nicht grün ist, trägt einen Grund in Klartext und eine Aktion, die ihn behebt. Kein Fehler ohne „Warum" und „Was jetzt". Das gilt für Sandbox-Start, TLS-Ablehnung, Timeouts, fehlenden Daemon, unerreichbaren LLM-Server, ungelöste Findings und Regel-Konflikte gleichermaßen.
8. **Wenig tun müssen, viel tun können.** Progressive Disclosure. Der Standardweg braucht drei Entscheidungen (LLM-Server, Projektordner, Start). Dahinter ist alles konfigurierbar: Agent, Sandbox-Profil, Mounts, Env, Netzwerkpfad zum LLM-Server, gespeicherte und temporäre Regeln, Timeouts, Detektoren, Katalog. Eine Konfigurationsquelle (TOML plus YAML-Regeln) speist UI und CLI gleichermaßen, das UI ist nie der einzige Weg.
9. **It just works.** Der Standardweg ohne Flags funktioniert: `humanitl run` nimmt das aktuelle Verzeichnis, den gefundenen LLM-Server und OpenCode aus dem PATH. Ein Paket installiert alles, der Erststart aktiviert den Hintergrunddienst mit einem Klick, `humanitl doctor` prüft die Maschine und liefert pro Zeile einen Fix. Fehlendes wird angeboten, nie nur gemeldet.
10. **CLI ist erstklassig.** `humanitl run` in einem Verzeichnis startet eine isolierte Agent-Instanz auch ohne UI. Das UI ist ein Client des Daemons, kein Gatekeeper.

---

## 2. Entscheidungen (ADR-Kurzform)

Jede Entscheidung hat eine eigene, ausformulierte Datei unter [`docs/adr/`](docs/adr/README.md) (Index dort). Hier die Zusammenfassung mit Begründung. Das Architektur-Leitbild (Schichten, Ports, Code-Sparsamkeit, Agent-Bewusstsein) steht in `docs/ARCHITECTURE.md`. Die ausgearbeiteten Issue-Spezifikationen liegen unter `backlog/sprint-N.md`, die gemeinsamen Namen in `backlog/CONVENTIONS.md`.

### ADR-001 Daemon in Rust auf hudsucker, nicht mitmproxy

- Sicherheit ist das Produkt. Ein statisches, speichersicheres Binary, das CA-Key, Aufzeichnungen und die Sandbox-Steuerung hält, ist eine kleinere Trusted Computing Base als ein Python-Bundle mit 40 transitiven Paketen.
- hudsucker liefert CONNECT, Zertifikats-Cache (rcgen), HTTP/1 und HTTP/2 über hyper 1, WebSocket-Handler, streamende Bodies mit Trailern. Der asynchrone `handle_request` ist der natürliche Haltepunkt (await auf einen oneshot-Channel).
- Die eigentliche Arbeit (Hold-Queue, Regeln, Recorder, Audit, gRPC, Sandbox) wäre in jeder Sprache selbst zu schreiben.
- Packaging als systemd user service ist trivial. mitmproxy wäre ein Packaging-Problem (Python-Runtime, C-Extensions, 80 bis 150 MB).
- Kosten: MITM-Randfälle sind unsere. Mitigation: Konformitäts-Matrix mit curl, websocat, grpcurl gegen einen Fake-Upstream ab Sprint 1. mitmproxy-Testkorpus und Regel-Semantik als Referenz nutzen.
- Protokoll-Ziel je Milestone, verbindlich: M1 = HTTP/1.1 client- und upstream-seitig (ALPN bietet dem Client nur `http/1.1`, Upstream wird auf h1 gezwungen), WebSocket-Passthrough, SSE. gRPC (braucht h2) ist in M1 ein dokumentierter Fehlschlag in der Matrix, nicht ein offenes Ziel; h2 client-seitig und upstream kommt in M6. Kein Sprint-Ziel darf „experimentell" sein.
- Listener: hudsucker lauscht auf TCP. Sprint 1 klärt in HUM-015 Schritt 0, ob hudsucker 0.25 einen generischen `Accept`-Stream annimmt; falls ja, lauscht der Proxy direkt auf dem Unix-Socket. Falls nein, wird hudsuckers Accept-Schleife (etwa 100 Zeilen) als Fork im Repo gehalten, damit die Tür bleibt, was sie sein soll: genau ein Unix-Socket, kein Loopback-Port auf dem Host, den andere Prozesse erreichen könnten.
- Verworfen: Go (goproxy veraltet, HTTP/2-MITM in net/http mühsam), Python/mitmproxy (siehe oben).

### ADR-002 bubblewrap zuerst, Docker später als zweites Backend

- Rootless, kein Daemon, Policy ist eine einzige lesbare `bwrap`-Kommandozeile, die das UI anzeigen kann.
- `--unshare-all` gibt einen leeren Netzwerk-Namespace (nur `lo`). Abstract Unix Sockets sind netns-gebunden, also ebenfalls tot.
- Docker `--network none` bräuchte trotzdem socat und seccomp im Container, läuft als root und hat mit `docker.sock` einen zusätzlichen Fußangel-Pfad. Kommt als `SandboxBackend`-Implementierung nach dem MVP, für vollständige Toolchain-Images.
- Verworfen für MVP: gVisor (rootless ohne Netstack), nsjail (kein Debian-Paket), Firecracker/Kata (KVM, Tooling-schwer; als v2 für stärkeres Threat Model vorgesehen).

### ADR-003 gRPC über Unix Domain Socket

- tonic auf Rust-Seite ist idiomatisch, Dart `grpc` 5.x unterstützt UDS via `InternetAddress.unix`. Server-Streaming liefert den Feed mit Backpressure durch HTTP/2-Flow-Control.
- Die `.proto` ist gleichzeitig Dokumentation, Versionsvertrag und spätere Plugin-Schnittstelle.
- Regeln: Protos langweilig halten (kein `Any`, wenig oneof-Verschachtelung), Dart-Codegen in CI mit gepinntem Plugin, `GetInfo` liefert Daemon- und Proto-Version, UI verweigert höhere Major.
- Daemon läuft als eigener Prozess, auch wenn Rust per FFI einbettbar wäre: Proxy muss die UI überleben, Flatpak kann kein bwrap ausführen, und ein CLI/Headless-Modus nutzt später denselben Vertrag.

### ADR-004 Request-Lebenszyklus als Zustandsautomat, Events abgeleitet

- `FlowState`: `Received → Analyzed{findings} → Held{deadline} → Decided(Allow | AllowEdited | Block | TimedOut) → Forwarded → Responded → Recorded`, plus `Failed{error}` erreichbar aus `Decided(Allow*)` und `Forwarded` für Upstream-Fehler nach der Freigabe (DNS, Connect, TLS, Timeout, private Adresse). `Failed` geht nach `Recorded`. Übergänge als Methoden, die `Result<(Self, FlowEvent), InvalidTransition>` liefern. Ein Upstream-Fehler wird nie als `Responded` verbucht; der Client bekommt `502`/`504` mit `Blocked by Humanitl`-Body und `reason: upstream_*`.
- Aus jedem Übergang wird ein `FlowEvent` erzeugt, der sowohl den gRPC-Stream als auch das Audit-Log speist. Kein Event Sourcing des In-Memory-Zustands, aber ein vollständiger Ereignisstrom nach außen.
- Held-Requests blockieren nie auf die UI: Hold-Queue ist `DashMap<FlowId, oneshot::Sender<Decision>>` plus Deadline-Timer.

### ADR-005 Vollständiges Puffern des Request-Bodys vor dem Halten

- Wer nur Header freigibt und den Body streamt, sieht genau den Teil nicht, in dem exfiltriert wird. Also: Request komplett puffern (Cap, Default 32 MB), dann anzeigen. Über dem Cap wird mit `413` geblockt (`BlockReason::BodyCap`), außer eine Regel sagt explizit `stream`.
- `Expect: 100-continue` beantwortet der Proxy sofort mit `100 Continue`: Der Body fließt dann in den Hold-Puffer des Proxys, nicht zum Upstream. Erst die Entscheidung öffnet den Weg nach draußen. (Korrektur nach Review: Die frühere Formulierung „erst nach Entscheidung" hätte das Puffern des Bodys unmöglich gemacht.)
- Statuscodes: `403` Policy (User, Regel, Authority-Mismatch, private Adresse), `413` Body-Cap, `504` Hold-Timeout, `502` Upstream-Fehler, `503` Hold-Speicherbudget erschöpft. Der Body beginnt immer mit `Blocked by Humanitl.` und trägt `reason:`, damit der Agent alle Fälle gleich erkennt.
- Hold-Speicherbudget ist global (`limits.hold_max_bytes`, Default 256 MiB, `limits.hold_max_flows` 200) und wird bereits in HUM-016 als einfache Zähler umgesetzt; darüber wird der neue Request mit `503` geblockt, nie ein gehaltener verworfen.
- Responses werden immer gestreamt (LLM-Streaming, SSE) und parallel in den Recorder gespiegelt.

### ADR-006 DNS-Auflösung erst nach Freigabe

- Ein Hostname wie `geheim-daten.attacker.com` leakt 63 Bytes pro Label, wenn der Proxy vor der Entscheidung auflöst. Der Hold arbeitet auf dem Hostnamen als String. Aufgelöst wird nur nach `allow`, einmal, über den `Resolver`-Port (hickory), nie über den System-Resolver eines HTTP-Connectors, und die IP wird über `Egress::connect(authority, ip)` gepinnt (kein DNS-Rebinding).
- Löst ein Name auf eine private, Loopback-, Link-Local- oder CGNAT-Adresse auf (RFC 1918, 127/8, 169.254/16, 100.64/10, fc00::/7, ::1), wird die Verbindung verweigert (`BlockReason::PrivateAddress`, Diagnostic `PROXY_005` mit Regelvorschlag), außer die matchende Regel trägt `allow_private: true`. Die LLM-Passthrough-Regel setzt das automatisch; `localhost` und `127.0.0.1` als LLM-Host funktionieren damit. So kann kein öffentlicher Name per Rebinding auf den Router, Cloud-Metadaten oder den LLM-Host zeigen.
- Domain-Vorschau im UI darf nie automatisch fetchen. Vorschau kommt aus einem gebündelten Katalog; Live-Fetch (Favicon, og:title) nur auf expliziten Klick, host-seitig, nur eTLD+1.

### ADR-007 Regel-Modell

- Geordnete Liste, first match wins, Default `ask`.
- Aktionen: `allow`, `block`, `ask`, `redact` (Pseudonymisierer laufen lassen, dann `allow` oder `ask`).
- Schlüssel: `host` (Glob auf Labels, nicht Substring: `*` = genau ein Label, `**` = ein oder mehr, blanker Host = exakt), `method`, `path` (Glob oder `~regex`), `scheme`, `port`. `expires`: `session` | Zeitstempel | `never`. `session` ist an die Sandbox-Instanz gebunden, nicht an die Uhrzeit. Einmaliges Erlauben ist keine Regel, sondern eine Entscheidung ohne Merken; deshalb gibt es kein `once`.
- Hostnamen werden auf A-Label (Punycode) normalisiert, lowercase, ohne trailing dot. IP-Literale matchen nie eine Host-Regel; private Bereiche und `169.254.169.254` brauchen eine explizite Regel.
- Nach TLS-Terminierung wird pro Request `Host`/`:authority` gegen das CONNECT-Ziel und SNI geprüft. Mismatch = block ohne Nachfrage (Domain Fronting).
- `allow_private: true` erlaubt private Zieladressen (siehe ADR-006); ohne das Flag werden sie geblockt.
- WebSocket-Upgrade ist eine eigene Aktion, Default `ask`; danach werden Frames aufgezeichnet, nicht angehalten. Das wird im UI ausgesprochen.
- Session-Regeln (temporär, In-Memory) werden vor persistenten Regeln ausgewertet; innerhalb jeder Gruppe gilt die Listenreihenfolge.
- Format YAML, Schema so gehalten, dass es später nach Cedar übersetzbar ist. Kein OPA/Rego.

### ADR-008 Speicherung

- SQLite (WAL) mit Migrationen. Bodies bis 256 KB inline als BLOB, darüber content-addressed unter `$XDG_DATA_HOME/humanitl/blobs/<sha256>`.
- Audit-Log als append-only JSONL mit Hash-Kette (`seq`, `ts`, `prev_hash`, `hash` über kanonisches JSON), HMAC mit Installationsschlüssel aus dem Keyring, periodisch verankerter Head-Hash (Anzeige im UI, zweiter Speicherort, zusätzlich per `logger` ins systemd-Journal). Ehrlich dokumentieren, was die Kette beweist und was nicht: Sie schützt gegen nachträgliches Editieren durch Dritte mit Dateizugriff, nicht gegen einen Angreifer, der als derselbe Nutzer läuft und Schlüssel plus Datei hat. Externer Anker ist Post-MVP.
- Pseudonymisierungs-Mapping getrennt, verschlüsselt, nur host-seitig, nie in der Sandbox, nie in der Anfrage.

### ADR-009 UI-Stack

- Das Chrome (Resizable, Command Palette, Sheet, Segmented, ContextMenu) kommt aus `shadcn_flutter`, exakt auf 0.0.54 gepinnt und ausschliesslich in `app/packages/ui` importiert. HUM-035 hatte am 2026-09-04 dagegen entschieden (88,3 % der gewichteten Punkte fuer die eigene Schicht gegen 48,3 % und 51,7 %); der Projekteigentümer hat das am selben Tag zurückgenommen, weil die ursprüngliche Fassung von ADR-0009 seine Vorgabe war. Die Bibliothek liefert Aussehen und Chrome, nicht die Verhaltensschicht: Kontrast, Fokus, Halten und Trefferflächen bleiben Sache von `packages/ui` (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer").
- Datenlastige Widgets aus Spezialpaketen, nicht selbst gebaut: `re_editor` (Editor), `xterm2` (Terminal), `diff_match_patch`. `two_dimensional_scrollables` steht noch aus und wird erst mit dem JSON-Baum entschieden (HUM-030); die History-Tabelle braucht es nicht: `ListView.builder` mit bekannter `itemExtent` gibt dieselbe Zusage und einen `Semantics`-Knoten je Zeile statt elf.
- riverpod 3 + Generator, freezed für Modelle, sealed classes für `FlowEvent`.
- Kein WebView auf Linux. Vorschau als Bild vom Daemon oder Katalog-Karte.
- Ein Fenster, gedockte Panes. Multi-Window ist in Flutter noch experimentell.

### ADR-011 Eine Konfigurationsquelle, drei Sichtbarkeitsstufen

- Alle Einstellungen sind Rust-Typen mit `serde` + `schemars`. Daraus entstehen: das TOML-Schema, die CLI-Flags (`clap`, gleiche Namen), die Settings-Oberfläche im UI (generiert aus dem Schema, mit Beschreibung, Default, Reset) und die Dokumentation. Kein Setting existiert nur im UI.
- Jedes Setting trägt eine Stufe: `basic` (Setup-Checkliste, drei Entscheidungen), `advanced` (Settings-Screen, standardmäßig sichtbar), `expert` (eingeklappt, mit Warnhinweis wo Sicherheitsrelevanz). Suche über alle Stufen.
- **Profile** bündeln: Sandbox-Profil, Regelsatz, Agent-Adapter, LLM-Endpoint, Timeout, Mounts, Env. Ein Profil ist eine Datei, kann pro Projekt (`.humanitl/profile.toml`) oder global liegen, Projekt gewinnt. `humanitl run --profile llm-only` und das UI nutzen dieselben Profile.
- Regeln haben zwei Ablageorte: gespeichert (`rules.yaml`, überlebt Neustart) und temporär (`expires: session`, nur im Daemon-Speicher, im UI als eigener Tab „Temporär" sichtbar, mit „dauerhaft machen").

### ADR-012 Geführte Zustände als Typ

- `Diagnostic { code, severity, title, why, fix: Option<FixAction>, docs: Option<Url> }` in `core-types`. Jeder Fehlerpfad im Daemon liefert ein `Diagnostic`, nie nur einen String. `FixAction` ist eine aufzählbare Aktion (Env-Variable setzen, Regel anlegen, Dienst installieren, Setting ändern, Befehl kopieren), die UI und CLI einheitlich rendern.
- Das UI zeigt Diagnostics immer am Ort des Problems (Karte, Zeile, Checklisten-Eintrag), nie als Modal. Die CLI zeigt sie als Block mit `why:` und `fix:`.
- Definition of Done für jedes Issue: neue Fehlerpfade haben ein `Diagnostic` mit `why` und, wo möglich, `fix`.

### ADR-013 CLI-Modus und Headless-Betrieb

- Binary `humanitl` (CLI) neben `humanitld` (Daemon). Subkommandos: `run [--profile X] [-- cmd]` (Agent im aktuellen Verzeichnis), `sandbox check|argv`, `rules list|add|test`, `flows list|show`, `audit verify|export`, `config get|set|schema`, `daemon install|status`.
- Ohne verbundenes UI gibt es zwei Modi für gehaltene Requests: `--ask terminal` (Prompt im Terminal, Muster pipelock: Host, Methode, Größe, Findings, dann `[a]llow [b]lock [r]ule`) oder `--ask none` (alles ohne Regel wird geblockt). `--ask terminal` ist nur für zeilenorientierte Kommandos (`humanitl sandbox run -- curl …`, Skripte, Aider im Basic-Mode) geeignet; bei Vollbild-TUI-Agenten wie OpenCode verweigert `humanitl run --ask terminal` den Start mit Diagnostic `CLI_002` und schlägt `--ask ui` (UI wird gestartet oder angehängt) oder `--ask none` vor. Das Profil `llm-only` setzt `--ask none` plus LLM-Passthrough, das ist die „nur Inferenz"-Instanz.
- Verbindet sich später ein UI, übernimmt es die Hold-Queue nahtlos, weil beide nur gRPC-Clients des Daemons sind.

### ADR-014 Agent-Bewusstsein über den einen Kanal

- Der Agent erfährt, wo er ist, über ein Briefing von etwa 150 Token, das der `AgentAdapter` als globale Instruktionsdatei in der Sandbox anlegt (OpenCode `~/.config/opencode/AGENTS.md`), nie im Projektverzeichnis.
- Feedback fließt durch den Proxy: Der Nutzer kann beim Blocken eine Notiz mitgeben, die im 403-Body (`note:`) und Header `X-Humanitl-Note` landet. Der Proxy bedient den virtuellen Host `humanitl.internal` selbst (`/`, `/why/<flow>`, `/ask`) ohne Upstream. `/ask` erzeugt nur eine Karte im UI, nie eine Regel.
- Kein neuer Kanal, keine neue Fähigkeit für den Agenten, keine Garantie-Lücke. Details in `docs/ARCHITECTURE.md` Abschnitt 8.

### ADR-015 Ports-and-Adapters mit erzwungener Abhängigkeitsrichtung

- Kern-Crates ohne IO, async und Proto; Anwendung kennt Adapter nur als Traits; Adapter außen. Abgeschlossene Port-Liste im MVP (`docs/ARCHITECTURE.md` Abschnitt 2), jeder Port hat genau eine Implementierung. Neuer Port nur mit ADR und konkretem zweitem Adapter.
- CI-Skript `tools/check-deps.sh` prüft den Cargo-Graphen gegen die erlaubte Richtung, `#![deny(missing_docs)]` in Bibliotheks-Crates, generierter Code gitignored.
- Code-Sparsamkeit: keine Abstraktion über Fremdbibliotheken, Werttypen statt Strings, Binaries sind nur Verdrahtung, `experimental`-Flags werden nach zwei ungenutzten Sprints entfernt.

### ADR-016 Browser für den Agenten über CDP, Zuschauen und Eingreifen im UI

- Sandbox-Profil `browser`: Chromium des Hosts (über den ro-Mount von `/usr`) und `nodriver` im Python-Pack. Start-Flags `--proxy-server=http://127.0.0.1:3128 --disable-quic --remote-debugging-port=9222 --headless=new --no-sandbox`, CA über `--ignore-certificate-errors-spki-list=<SPKI-Hash der Humanitl-CA>`. Ein gebündeltes `humanitl_browser.py` liefert die fertige nodriver-Konfiguration. Alle Seitenaufrufe laufen durch den Proxy wie jeder andere Request.
- Zuschauen: Der Shim startet vor seccomp eine zweite, host-initiierte Bridge (Unix-Socket in der Sandbox, weiter auf `127.0.0.1:9222`). Der Daemon spricht CDP (`chromiumoxide`), `Page.startScreencast` liefert Frames, gRPC-Stream `Browser` bringt sie ins UI. Eingreifen: Maus und Tastatur gehen als CDP-Input zurück, Übernahme-Modus sichtbar, Agent und Mensch können gleichzeitig dran (Chromium erlaubt mehrere CDP-Clients).
- Sicherheitsfolgen: Der Shim unterstützt eine Bridge-Liste aus dem Profil. seccomp erlaubt `AF_INET`/`AF_INET6` immer (nur Loopback existiert); das Profil `browser` erlaubt zusätzlich `AF_UNIX`, weil Chromium Socketpairs für seine IPC braucht. Die Garantie trägt der Netzwerk-Namespace plus Mount-Allowlist; seccomp bleibt doppelter Boden. Chromium läuft ohne eigene Sandbox (`--no-sandbox`), weil bwrap die User-Namespaces belegt; bwrap ist die äußere Sandbox.
- Post-MVP (M7). Im MVP nur die Vorarbeiten: Bridge-Liste im Shim (HUM-012), seccomp-Familien pro Profil (HUM-010).

### ADR-017 Ein Egress-Port für Direktverbindung, Upstream-Proxy und Tor

- Der Proxy öffnet jede Upstream-Verbindung über genau einen Port `Egress { fn connect(&self, authority) -> Stream }`. MVP: `Direct` (Resolver nach Allow, IP gepinnt). Später: `HttpProxy(url)` (CONNECT an Firmen-Proxy) und `Socks5h(url)` (Tor unter `socks5h://127.0.0.1:9050`), global über `egress.via` oder pro Regel über `via:`.
- Bei `Socks5h` wird der Hostname unaufgelöst an den SOCKS-Proxy übergeben; lokale DNS-Auflösung ist in diesem Modus verboten (DNS-Leak). Der Isolation-Check bekommt eine vierte, optionale Zeile „Egress: Tor" mit Prüfung gegen `check.torproject.org`.
- Post-MVP, Nice-to-have. Im MVP nur: alle Upstream-Connects laufen durch den Port (HUM-015), keine direkten `TcpStream::connect`-Aufrufe außerhalb.

### ADR-018 Parität: Jede Fähigkeit ist zuerst ein RPC, UI und CLI sind dünne Clients

- Es gibt genau eine Schnittstelle zum Kern: die gRPC-Proto `humanitl.v1`. Jede Funktion (Entscheiden, Regeln, Sandbox, Terminal, Doctor, Discovery, Config, Audit, Export) wird zuerst als RPC entworfen und im Daemon implementiert. UI und CLI rufen nur RPCs auf und enthalten keine Fachlogik.
- GUI-first in der Reihenfolge der Umsetzung, aber jedes Issue, das einen neuen RPC einführt, liefert im selben Issue das CLI-Subkommando mit (auch wenn es nur `--json` ausgibt). Umgekehrt bekommt jedes CLI-Subkommando eine UI-Entsprechung spätestens im Folgesprint.
- Paritäts-Tabelle `docs/reference/parity.md`, generiert von `cargo xtask docs`: RPC, CLI-Subkommando, UI-Ort. CI schlägt fehl, wenn ein RPC ohne CLI-Zeile existiert.
- Gute Defaults sind Teil der Parität: Das mitgelieferte Profil `default` beschreibt den gängigen Anwendungsfall vollständig (OpenCode, LLM im LAN, `ask`, 5 Minuten Timeout, gebündelte Block-Regeln, Session-Regeln bevorzugt). Wer nichts ändert, bekommt diesen Weg; wer ändern will, findet jeden Wert in Profil, Settings-Screen und CLI unter demselben Schlüssel.

### ADR-010 Packaging

- `.deb` und AppImage zuerst (fastforge), Daemon-Binary liegt neben dem Flutter-Bundle, systemd user unit wird beim ersten Start angeboten.
- Flatpak später: nur die UI im Flatpak, Daemon außerhalb, Verbindung über `--filesystem=xdg-run/humanitl`.

---

## 3. Architektur

### 3.1 Überblick

```
+--------------------------------+      gRPC über UDS       +------------------------------------+
| humanitl (Flutter, GTK)        | <----------------------> | humanitld (Rust, systemd --user)   |
|  Intercept · History · Rules   |  Stream: FlowEvent       |  proxy     Hold-Queue, MITM        |
|  Sandbox · Audit               |  Cmds: Decide, Rules,    |  rules     reine Matching-Logik    |
|  Terminal (xterm2 via Stream)  |        Sandbox, Query    |  findings  Regex-Detektoren        |
+--------------------------------+                          |  recorder  SQLite + Blobs          |
                                                            |  audit     Hash-Kette              |
                                                            |  sandbox   Backend-Trait, bwrap    |
                                                            |  ipc       tonic-Server            |
                                                            +-----------------+------------------+
                                                                              | genau ein UDS (Datei-Bind)
                                                            +-----------------v------------------+
                                                            | Sandbox: bwrap --unshare-all       |
                                                            |  shim: socat-Bridge, dann seccomp, |
                                                            |        dann exec Agent             |
                                                            |  Agent (opencode) HTTP_PROXY=      |
                                                            |        127.0.0.1:3128              |
                                                            |  /work  (Projekt, ro|rw)           |
                                                            |  CA nur als .crt, Env-Kit gesetzt  |
                                                            +------------------------------------+
                                                                              |
                                                     LAN-LLM (Passthrough-Regel, geloggt)  ·  Internet (nur nach Freigabe)
```

### 3.2 Monorepo-Layout

```
humanitl/
  proto/humanitl/v1/*.proto          Einzige Quelle der Wahrheit für IPC
  daemon/                             Cargo-Workspace
    crates/core-types/                Typisierte IDs (UUIDv7), FlowState, FlowEvent, Fehler-Taxonomie
    crates/rules/                     Rein, ohne IO: Parsen, Normalisieren, Matchen
    crates/findings/                  Detektor-Trait + Regex/Checksum-Detektoren
    crates/recorder/                  SQLite, Blob-Store, Migrationen
    crates/audit/                     Hash-Kette schreiben und prüfen
    crates/sandbox/                   SandboxBackend-Trait, bwrap-Impl, seccomp (seccompiler)
    crates/proxy/                     hudsucker-Wrapper, Hold-Queue, Zustandsautomat
    crates/ipc/                       tonic-Server, Proto <-> Domain-Mapping
    crates/catalog/                   Domain-Katalog, Public Suffix List, Verbreitungsrang
    crates/config/                    Settings-Typen mit serde + schemars, Stufen basic/advanced/expert, Profile
    bin/humanitld/                    Verdrahtung, Config, tracing
    bin/humanitl/                     CLI: run, sandbox, rules, flows, audit, config, daemon
    bin/humanitl-shim/                50-Zeilen-Launcher in der Sandbox (Bridge-FD erben, seccomp, exec)
  profiles/llm-only.toml, default.toml   Benannte Profile (Sandbox + Regeln + Agent + LLM + Timeout)
  app/                                Flutter, feature-first
    lib/core/{ipc,domain,ui}          generated/ ist gitignored, CI erzeugt
    lib/features/{setup,intercept,editor,history,rules,sandbox,audit,settings}
    packages/ui/                      Widget-Vokabular auf reinem Flutter
    l10n/{app_en.arb,app_de.arb}
  profiles/sandbox/*.toml             bwrap-Argv-Templates, Env-Kit, Mount-Allowlist
  agents/opencode/                    Profil, gebündelte models.json, opencode.json-Template
  catalog/domains.yaml                ~200 Dev-Services mit Icon, Kategorie, Beschreibung
  catalog/public_suffix_list.dat
  rules/default.yaml                  Vorab-Regeln (Blocks für bekannte Phone-Home-Hosts)
  packaging/{systemd,deb,appimage}
  tests/escape/                       Escape-Tests (laufen in der Sandbox)
  tests/e2e/                          Echter Daemon + Fake-Agent + UI unter xvfb
  docs/{SECURITY.md,THREAT-MODEL.md,DESIGN.md,adr/}
```

Modulgrenzen: `rules`, `findings`, `audit` haben kein async und kein IO, sind tabellengetrieben testbar. `proxy` kennt nur `core-types`. `ipc` ist die einzige Crate, die Protobuf kennt.

### 3.3 IPC-Vertrag (Skizze)

```
service Humanitl {
  rpc GetInfo(Empty) returns (Info);                       // daemon_version, proto_version, capabilities
  rpc Subscribe(SubscribeRequest) returns (stream FlowEvent);
  rpc ListFlows(ListFlowsRequest) returns (FlowPage);       // Filter, Sort, Cursor; serverseitig in SQLite
  rpc GetFlow(FlowId) returns (FlowDetail);                 // inkl. Body-Referenz
  rpc GetBody(BodyRef) returns (stream BodyChunk);
  rpc Decide(DecideRequest) returns (DecideResponse);       // Allow | AllowEdited{request} | Block, remember?: Rule
  rpc Rules(RulesRequest) returns (RulesResponse);          // list, add, update, remove, reorder, dry_run
  rpc Sandbox(SandboxRequest) returns (stream SandboxEvent);// start, stop, status, isolation_check
  rpc Terminal(stream TerminalInput) returns (stream TerminalOutput);
  rpc Audit(AuditRequest) returns (AuditResponse);          // verify, export, head_hash
}
```

`FlowEvent` ist ein oneof über `Received`, `Analyzed`, `Held`, `Decided`, `Forwarded`, `ResponseHeaders`, `ResponseChunk`, `Recorded`, `TimedOut`, `Lagged{n}`, `Diagnostic`, `RulesChanged`, `AgentAsk`. Bei `Lagged` synchronisiert die UI über `ListFlows(since)`. Zusätzlich `GetConfig`/`SetConfig`. Die vollständige Proto steht in `backlog/sprint-0.md` (HUM-003) mit den Erweiterungen aus `backlog/CONVENTIONS.md` 4.3.

### 3.4 Datenmodell (SQLite)

- `sessions(id, started, ended, sandbox_profile, llm_endpoint)`
- `flows(id, session_id, ts, method, scheme, host, port, path, state, decision, rule_id, duration_ms, edited)`
- `messages(flow_id, dir, headers_json, body_inline, blob_ref, size, truncated)`
- `rules(id, pos, yaml, created_from_flow, created, expires)`
- `findings(flow_id, kind, span_start, span_end, value_hash, tier, resolved)`
- `pseudonyms(session_id, pseudonym, kind, value_hash, value_encrypted, first_seen, count)`

### 3.5 Laufzeit-Konventionen

- Config TOML unter `$XDG_CONFIG_HOME/humanitl/config.toml`, Regeln daneben in `rules.yaml`, Daten unter `$XDG_DATA_HOME/humanitl`, Socket unter `$XDG_RUNTIME_DIR/humanitl/daemon.sock` (0600, Verzeichnis 0700), Proxy-Socket in einem eigenen Verzeichnis, wird als Datei gebunden, nie das Verzeichnis.
- Logging mit `tracing`, JSON nach journald. OpenTelemetry als Cargo-Feature, nicht im MVP.
- Fehler: `thiserror` pro Crate, eine `DaemonError` auf Bin-Ebene mit stabilem `code`-String, Mapping auf gRPC-Status.
- Feature-Flags: Cargo-Features `docker`, `otel`; Laufzeit-Tabelle `[experimental]` in der Config für h2-Upstream, WebSocket-Hold.
- Große Bodies in der UI werden in `Isolate.run` geparst, LRU-gecacht. History-Tabelle hält nur Summaries, Bodies auf Selektion.

---

## 4. Sicherheitsmodell

Vollständige Fassung nach Sprint 0 in `docs/SECURITY.md` und `docs/THREAT-MODEL.md`. Hier die verbindliche Kurzform.

### 4.1 Die drei Garantien (live prüfbar im UI)

1. **Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.** `bwrap --unshare-all --cap-drop ALL`. In der Sandbox existiert nur `lo`. Keine IP, kein DNS, kein ICMP, kein QUIC, keine Raw Sockets. Prüfung: `ip link` zeigt nur `lo`.
2. **Genau eine Tür: ein Socket, der zu Humanitl führt.** Der Proxy-Socket des Daemons wird als einzelne Datei in die Sandbox gebunden. Prüfung: `find / -type s` liefert genau eine Datei.
3. **Der Kernel öffnet keine neue Tür (seccomp).** Der Shim hält die Bridge selbst (kein socat), setzt vor `exec` des Agenten einen seccomp-Filter, der `socket()` nur noch für `AF_INET`/`AF_INET6` mit Typ `SOCK_STREAM` erlaubt (nötig für Loopback zum Proxy; im Namespace existiert nur `lo`, Routing-Tabelle leer, alle Capabilities gedroppt) und alle anderen Familien (`AF_UNIX`, `AF_NETLINK`, `AF_PACKET`, `AF_VSOCK`, …) sowie `SOCK_RAW`/`SOCK_DGRAM` mit `EPERM` ablehnt; zusätzlich `ptrace`, `io_uring_*`, `process_vm_*`, `keyctl` und alle x32-Syscalls (Bit `0x40000000`). Prüfung: `Seccomp: 2` in `/proc/<agent>/status`, `socket(AF_UNIX)` und `socket(AF_INET, SOCK_DGRAM)` liefern `EPERM`, `socket(AF_INET, SOCK_STREAM)` gelingt, `connect` an eine LAN-Adresse scheitert mit `ENETUNREACH`.

Zusätzlich: `--unshare-pid --unshare-ipc --unshare-uts`, eigenes `/proc`, `--tmpfs /dev/shm`, `--new-session`, `--die-with-parent`, Mount-Allowlist (nie `$XDG_RUNTIME_DIR`, `/tmp`, `/run`, `~/.ssh`, `~/.gitconfig`, `~/.netrc`, X11/Wayland/dbus/docker-Sockets).

### 4.2 Deklarierte Seitenkanäle (werden im UI gezeigt, nicht versteckt)

| Kanal | Warum er existiert | Umgang im MVP | Später |
|---|---|---|---|
| **LLM-Passthrough** | Der Agent muss legitim Code ans LLM schicken | Regel exakt auf Host+Port+Pfadpräfix+POST, gestreamt, vollständig geloggt, Findings-Scan mit Warnung, amber im Isolation-Panel. Dokumentiert: „LLM-Host ist Teil der Vertrauensbasis" | Findings-basiertes Halten, Rate-Limits |
| **`/work` (rw)** | Der Agent muss Dateien schreiben | Nur Unterpfad mounten, `.git/hooks`, `.git/config`, `.envrc`, `.vscode`, `.idea` maskiert (tmpfs). Session-Ende: Diff-Zusammenfassung „berührte Dateien", Secret-Scan über den Diff, Symlinks nach außerhalb `/work` markiert | Git-Operationen host-seitig aus dem UI |
| **Terminal-Ausgabe** | Der Mensch liest, was der Agent schreibt | xterm2: OSC 52 (Clipboard), OSC 8, Titel-Setzen deaktiviert. Banner „Agent-Ausgabe ist nicht vertrauenswürdig" | |
| **Hostname im Log** | Blockierte Anfragen sind trotzdem sichtbar | Dokumentiert. Der Export trägt Hosts, Pfade, Kopfzeilen und Rümpfe im Klartext und sagt das vor dem Schreiben; die optionale Host-Redaktion kommt nach dem MVP (`docs/SECURITY.md`) | |
| **Package-Caches** | Wiederholte Installs | Cache-Volumes pro Projekt, nicht pro Vertrauensstufe | Read-only Seeds |

### 4.3 Threat Model (Kurz)

Angreifer ist Code, der als Agent läuft, mit unterwanderter Absicht: (a) Prompt Injection über geladene Webseiten, READMEs, Issues; (b) bösartige Abhängigkeit, die der Agent ausführt; (c) kompromittiertes lokales Modell.

Gestoppt: direkter Netzwerk-Egress (a, b, c), Credential-Diebstahl aus Host-Env (PID-Namespace), Lesen beliebiger Host-Dateien (Mount-Allowlist), DNS-Leak vor Entscheidung (ADR-006), Domain Fronting (ADR-007).

Nicht gestoppt und deshalb sichtbar gemacht: Exfiltration über `/work` (alle drei), über den LLM-Prompt (a, b; bei c ist das LLM selbst der Angreifer), der Mensch als Klassifikator (ein base64-Body an `api.github.com` sieht legitim aus, deshalb Findings prominent).

### 4.4 Proxy-Härtung

- CA-Key pro Installation, `0600`, nur `ca.crt` in die Sandbox, nie in den Host-Trust-Store. Option: ephemere CA pro Session.
- systemd-Unit-Härtung: `NoNewPrivileges`, `ProtectHome=read-only` + `ReadWritePaths`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, `PrivateTmp`, `SystemCallFilter=@system-service`.
- Body-Caps für Dekodierung und Vorschau (8 MB), Dekompressions-Ratio-Limit, Timeouts. hyper/h2 gepinnt und regelmäßig aktualisiert (Rapid Reset, CONTINUATION-Flood). Fuzzing des Parsers und der Decoder in CI.

### 4.5 Escape-Tests (CI, müssen alle „fehlschlagen" im Sinne von: nichts kommt raus)

1. **Socket-Verweigerung.** `socket()` für AF_UNIX, AF_NETLINK, AF_PACKET, AF_VSOCK sowie `socket(AF_INET, SOCK_DGRAM|SOCK_RAW)`: `EPERM`; `socket(AF_INET, SOCK_STREAM)` gelingt, aber `connect` an `10.0.0.1:80` scheitert mit `ENETUNREACH`; `io_uring_setup`: `EPERM`; ein x32-Syscall (`syscall(0x40000000 | SYS_getpid)`): `EPERM`. `ip link` nur `lo`, Routing-Tabelle leer, `capsh --print` zeigt keine Capabilities. `Seccomp: 2` für Shim-Kind und Agent.
2. **Mount-Oberfläche.** `/proc/self/mountinfo` enthält keine X11/Wayland/dbus/docker-Sockets, kein `/run/user`, kein `/home` außer `/work`. `find / -type s` = genau der Proxy-Socket. `/proc/1/environ` des Hosts unlesbar. Hostname ist `sandbox`.
3. **Nur-Proxy-Egress.** `curl http://example.com` ohne Proxy: kein Route. Über Proxy: `blocked.example`, `10.0.0.1`, `169.254.169.254`, Homograph `xn--80ak6aa92e.com`, `-H 'Host: evil.io'` gegen CONNECT `github.com`: landen in der Hold-Queue mit exakt dem vorhergesagten Host, und der Host-Resolver zeigt null Lookups vor der Entscheidung.
4. **Regel-Tabelle.** `*.github.com` gegen `api.github.com` ✓, `github.com` ✗, `evil-github.com` ✗, `github.com.evil.io` ✗, `a.b.github.com` ✗, `API.GITHUB.COM.` ✓; `**`-Varianten; IP-Literal matcht nie; `ws://` auf erlaubten Host ergibt `ask`; 40-MB-POST wird gepuffert und gehalten oder über Cap geblockt, nie gestreamt.
5. **Dateisystem und Seitenkanäle.** Symlink `/work/x -> /home` wird in der Session-Zusammenfassung markiert, `.git/hooks/pre-commit` landet auf maskiertem Pfad, OSC-52-Escape verändert das Host-Clipboard nicht. Audit-Tamper: Eintrag löschen, Tail kürzen, re-verify erkennt beides (Tail-Fall bleibt rot, bis Head-Anchoring existiert).

---

## 5. Design-Richtung „Airlock"

Vollständig nach Sprint 0 in `docs/DESIGN.md`. Verbindliche Eckpunkte:

**Stimmung.** Ruhiger Kontrollraum. Jede Anfrage ist ein Paket in einer Schleuse; das UI macht das Warten sicher und die Entscheidung offensichtlich. Dicht, monochrom, präzise, Farbe nur bei Zustandswechsel. Referenzen: Linear (getönte Neutralfläche, 13px-Dichte, Keyboard-first), Raycast (Command Palette als primärer Handlungsweg), Zed (Hairlines statt Schatten, ruhige Statusleiste), Little Snitch (Allow/Deny × Dauer × Ziel, Countdown). Nicht übernehmen: Burp/ZAP-Lärm, Neon, Terminal-Grün, Modal-Dialoge.

**Farbe.** Dark-first, kühl getönte Neutrale (Hue ~230): `bg-0` #0F1115, `bg-1` #151821, `bg-2` #1B1F2A, `bg-3` #232838, `line` #2A3040, `fg-0` #E6E8EE, `fg-1` #A3A9B8, `fg-2` #6B7186. Ein Akzent: #7C9CF5 (Fokus, Primärbutton, Selektion, Links, sonst nichts). Zustände: held #E0B24A, allowed #4FBF8C, allowed-edited grün + Akzent-Stiftpunkt, blocked #E5646E, timed-out #8A90A2, auto-rule grün 60 %, LLM-passthrough #B48AF0, error/secret #F0784F (orange, damit Rot ausschließlich „blockiert" bedeutet). Tints maximal 10 % Alpha. Light-Theme durch Invertieren der Neutral-Leiter, Zustandsfarben 12 % dunkler.

**Typografie.** Inter (UI, `tnum`), JetBrains Mono (URLs, Header, Bodies, Hashes, Regeln, Method-Badges), Ligaturen aus. Skala 11/16, 12/16, 13/20, 14/22, 16/24, 20/28. Gewichte 400/500/600, kein 700.

**Layout.** Basis 4px, Panel-Padding 12px, Radius 4px Controls / 6px Cards / 0 Panels. Intercept drei Panes 28/44/28 mit Minimalbreiten 280/480/260. Header 40px, Statusleiste 24px. Queue-Zeile 36px (Zustands-Rail 4px, Method-Badge, Host 13/500, Pfad Mono 12 mittig gekürzt, Countdown, Findings-Chip), selektiert 56px mit Zweitzeile.

**Informationsarchitektur.** Linke Icon-Rail (Intercept, History, Rules, Sandbox, Audit; `Ctrl+1..5`), Command Palette `Ctrl+K`. Pane = persistent und resizable. Sheet (rechts) = Regel aus Anfrage, Detail aus History, Isolation-Details. Modal = nur destruktiv (Block-all > 5, Forever-Regel löschen, Sandbox stoppen bei laufendem Agent). Intercept-Queue und History sind zwei Projektionen desselben `Flow`-Streams (Queue = `state == held`), kein Toggle.

**Interaktion.** Enter = Allow einmal unverändert. `A` allow, `B` block, `E` edit, `R` Scope-Popover, `Ctrl+F`/`Ctrl+L` Burp-kompatibel, `Ctrl+Shift+F` Allow-Gruppe, `J/K` navigieren, `1/2/3` Dauer, `Shift+1..4` Ziel, `/` Filter, `Ctrl+K` Palette. Allow und Block nie benachbart. Neue Anfragen stehlen nie den Fokus, Liste wird nie unter dem Cursor umsortiert („+3 seit du liest"). Gruppierung nach Host, Batch-Button heißt „Allow 14 → registry.npmjs.org", nie „Allow all". Vor „Merken" wird die Regel als Satz gezeigt: `allow · GET · *.npmjs.org · diese Session`. Default-Dauer ist Session, nicht Forever. Regel erstellt: Inline „Regel gespeichert · Rückgängig", kein Toast. Auto-Allow durch Regel: nur Feed-Zeile mit Regel-Chip, Zähler in der Rail. Queue 0 → 1: Desktop-Notification mit Allow/Block-Aktionen, Tray-Badge, Rückkehr-Banner „Agent wartet seit 4 min".

**Signature-Elemente.**
1. *Release Valve*: Allow ist ein geteilter Pill. Links „Allow" (einmal), rechts Chevron öffnet Dauer × Ziel-Raster. 400 ms Halten links füllt grün und zeigt „Allow für Session".
2. *Isolation Ring*: 20px-Ring um das Sandbox-Glyph im Header, drei Segmente für die drei Garantien, Klick öffnet Isolation-Panel mit exakter bwrap-Kommandozeile. Vierte Zeile amber: „Ausnahme: LLM unter 192.168.1.20:11434, Passthrough, geloggt, nicht angehalten."
3. *Diff-Glow*: ersetzte Spans im Editor mit 1px Akzent-Unterstrich und 10 % Fläche, Hover zeigt Original ↔ Pseudonym (host-seitig).

**Motion.** Easing `cubic-bezier(0.2,0,0,1)`, Exits `cubic-bezier(0.4,0,1,1)`. Ankunft: 8px Slide + Fade 180 ms. Countdown: Ring um Glyph, unter 20 % atmende Opazität, kein Wechsel auf Rot. Entscheidung: Button füllt 120 ms, Rail sweept 200 ms. Verlassen: 220 ms Collapse, erlaubte Karten gleiten nach rechts, geblockte nach links.

**Sprache.** Englisch als Quellsprache, Deutsch ab Tag eins. Deutsch: „angehalten" (nicht „abgefangen"), Button „Senden" vs Regel „Erlauben", „Blockieren", „Gültigkeit" (Dauer) und „Ziel" (Scope), „Merken", „Pseudonymisieren" (nicht „Anonymisieren", rechtlich anders). Protokollbegriffe (Header, GET, Allow in Regeln) bleiben Englisch.

**Anti-Patterns.** Rot als Ambiente, Modals für Entscheidungen, Toast-Spam, Hit-Targets unter 28px, Schatten-Stapel, Hosts am Ende kürzen, automatisches Favicon-Laden, Dashboard-Gradienten.

---

## 6. Erweiterbarkeit (vormerken, nicht bauen)

Plugins sind kein MVP-Ziel. Damit sie später ohne Umbau möglich sind, gelten ab Sprint 0 diese Schnitte:

| Erweiterungspunkt | Form im MVP | Späterer Plugin-Weg |
|---|---|---|
| Findings-Detektoren | `trait Detector { fn scan(&self, body: &[u8], ct: &ContentType) -> Vec<Finding> }`, Registry, Regex-Detektoren aus TOML (gitleaks-Format) | Externe Detektoren als Prozess über gRPC (`DetectorService`), WASM-Detektoren (wasmtime) |
| Regel-Aktionen | `enum Action { Allow, Block, Ask, Redact }` plus `trait ActionHandler` | Custom-Aktionen (`webhook`, `script`) |
| Domain-Katalog | YAML-Dateien, mehrere Quellen gemergt | Nutzer-Katalog, Team-Katalog |
| Sandbox-Backends | `trait SandboxBackend { fn launch(profile) -> Handle; fn isolation_check() }` | Docker, Podman, microVM, macOS Seatbelt |
| Agent-Adapter | `trait AgentAdapter { env kit, default rules, profile, permission bridge? }`, Impl OpenCode | Aider, Codex, Claude Code, eigene |
| UI-Panels | Feature-Module hinter riverpod-Providern, Domain-Panel als Slot | Panel-Plugins über gRPC-Events, später Dart-Packages |
| Öffentliche API | Die gRPC-Proto selbst, Token-authentifiziert | Externe Prozesse als Plugins (Subscribe + Decide + Rules) |
| Diagnostics und Fixes | `Diagnostic`/`FixAction` als Enum | Plugins liefern eigene Diagnostics mit eigenen Fix-Aktionen |
| Profile | TOML-Dateien, global oder pro Projekt | Team-Profile, Profil-Import aus URL nach Prüfung |

Regel: kein Erweiterungspunkt bekommt im MVP mehr als ein Trait, eine Registry und eine Implementierung.

---

## 7. Roadmap: Milestones

| Milestone | Name | Ergebnis, demonstrierbar |
|---|---|---|
| M0 | Fundament | Monorepo, CI grün, Proto v1, Fake-Daemon spielt Session ab, Escape-Test-Harness, SECURITY.md-Entwurf, Design-Tokens |
| M1 | Sealed Box | Sandbox nachweislich dicht (Escape-Tests 1 bis 3 grün, 4 und 5 ab M2), `curl` aus der Sandbox landet als gehaltener Request im Daemon. Noch keine dauerhafte Aufzeichnung (Recorder kommt M2); der Sicherheitssatz „alles wird aufgezeichnet" gilt ab M2 |
| M2 | First Decision | Vollständiger Moderationskreislauf mit echtem UI: Queue, Karte, Allow/Edit/Block, Merken mit Scope, Regeln, History, Notification |
| M3 | Agent Inside | OpenCode läuft in der Sandbox gegen Ollama im LAN, Terminal im UI, Isolation-Check-Panel, Default-Regeln, Setup-Screen, `humanitl run --profile llm-only` liefert eine reine Inferenz-Instanz im aktuellen Verzeichnis |
| M4 | Trusted Editor | Pseudonymisierungs-Editor mit Findings, Mapping, Audit-Hash-Kette, Export, DE/EN, Settings-Screen mit Progressive Disclosure, Packaging deb + systemd |
| M5 | MVP 0.1 | Härtung, Fuzzing, Docs, e2e-Demo-Skript in CI, Release |

Post-MVP-Milestones (Abschnitt 9): M6 Docker-Backend + HTTP/2-Upstream, M7 Browser (nodriver, CDP-Screencast, Eingreifen), M8 Plugin-API, M9 Response-De-Pseudonymisierung, M10 Domain-Vorschau Live + Screenshot, M11 Credential-Injection, M12 macOS, M13 microVM.

---

## 8. Sprints und Issues

Sprints à 2 Wochen. Schätzung: S ≤ 1 Tag, M 2–3 Tage, L 4–5 Tage. Jeder Sprint endet mit einem grünen Demo-Skript in CI, sonst wird nichts anderes gemerged. IDs `HUM-xxx` werden später 1:1 zu GitHub-Issues. Nummern sind nicht chronologisch, die Reihenfolge in der Tabelle ist die Reihenfolge der Bearbeitung.

Definition of Done für jedes Issue: Tests auf der passenden Ebene, neue Fehlerpfade liefern ein `Diagnostic` mit `why` und wo möglich `fix`, neue Settings haben Stufe, Beschreibung und Default im Schema, neue Strings in `en` und `de`.

### Sprint 0 — Fundament (M0)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-001 | Monorepo anlegen | S | Layout aus 3.2, `cargo build` und `flutter build linux` laufen leer durch, `.editorconfig`, `rust-toolchain.toml`, Flutter-Version gepinnt, `CONTRIBUTING.md` |
| HUM-002 | CI-Pipeline | M | Jobs `rust-check` (fmt, clippy, cargo-deny), `rust-test`, `proto-lint-and-gen` (buf, protoc-gen-dart, Fehler bei Drift), `flutter-analyze-test`, `escape-tests` (Runner mit bwrap), `e2e-xvfb` (Platzhalter), alle grün auf leerem Stand |
| HUM-003 | Proto v1 definieren | M | Alle RPCs aus 3.3, `FlowEvent` oneof, `Info` mit Versionen, buf lint sauber, Dart- und Rust-Codegen in CI |
| HUM-004 | core-types Crate | M | `FlowId`/`RuleId`/`SessionId`/`SandboxId` als UUIDv7-Newtypes, `FlowState`-Automat mit `Result<Self, InvalidTransition>`, `FlowEvent`-Ableitung, Fehler-Taxonomie, Unit-Tests für alle Übergänge inklusive verbotener |
| HUM-005 | Fake-Daemon für UI-Entwicklung | M | `humanitld --fake <session.jsonl> [--speed N]` implementiert dieselbe gRPC-Schnittstelle in Rust, spielt eine JSONL-Session mit Timestamps ab, hält Flows echt (Deadline, Timeout); dazu `FakeDaemonClient` in Dart für Widget-Tests (`--dart-define=HUMANITL_FAKE=<scenario>`) |
| HUM-006 | Escape-Test-Harness | M | Skript startet ein bwrap-Profil und führt Test 1–3 aus 4.5 aus, Ergebnis als JUnit-XML, läuft in CI (erwartet noch rot bis M1) |
| HUM-007 | SECURITY.md und THREAT-MODEL.md Entwurf | S | Drei Garantien, deklarierte Seitenkanäle, Threat Model aus Abschnitt 4 in ausformulierter Fassung |
| HUM-008 | Design-Tokens und `packages/ui` | M | Farb-, Typo-, Spacing-Tokens aus Abschnitt 5 als Dart-Konstanten, Dark+Light-Theme, Wrapper für Button, Badge, Pill, Panel, Row; Storybook-artige Galerie-Seite; Inter und JetBrains Mono als Familien mit Fallback-Stack |
| HUM-009 | ADR-Verzeichnis | S | `docs/adr/0001` bis `0010` aus Abschnitt 2, Template für neue |
| HUM-010 | Sandbox-Profil-Format | S | `profiles/sandbox/*.toml`: bwrap-Argv-Template, Mount-Allowlist, Env-Kit, maskierte Pfade; Parser mit Tests |
| HUM-062 | config Crate mit Schema | M | Settings-Typen mit `serde` + `schemars`, Stufen-Attribut `basic`/`advanced`/`expert`, Beschreibung und Default pro Feld, JSON-Schema-Export, Laden aus TOML mit Overrides (global, Projekt `.humanitl/`, CLI-Flag, Env), Tests für Präzedenz |
| HUM-074 | Abhängigkeits-Lint | S | `tools/check-deps.sh` liest `cargo metadata`, prüft jede Workspace-Abhängigkeit gegen die Richtungstabelle aus `backlog/CONVENTIONS.md` 3.1, schlägt bei Verstoß fehl; `#![deny(missing_docs)]` in allen Bibliotheks-Crates; CI-Job `deps-lint` |
| HUM-063 | Diagnostic-Typ | S | `Diagnostic`, `Severity`, `FixAction`-Enum in `core-types`, Proto-Abbildung, Renderer-Vertrag für UI und CLI, Lint im CI: kein `Err(String)` in öffentlichen Daemon-Pfaden |

### Sprint 1 — Sealed Box (M1)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-011 | bwrap-Launcher | M | `SandboxBackend`-Trait, bwrap-Impl aus Profil, `--unshare-all --new-session --die-with-parent`, eigenes `/proc`, `/dev`, `tmpfs /dev/shm`, Hostname `sandbox`, Argv wird als String exponiert |
| HUM-012 | humanitl-shim | M | Startet socat-Bridge `127.0.0.1:3128 -> /run/proxy.sock`, wartet auf Listen, setzt seccomp-Filter (seccompiler: `socket`, `socketpair` alle Familien EPERM, `PR_SET_NO_NEW_PRIVS`, TSYNC), `exec` des Agent-Kommandos. Escape-Test 1 grün |
| HUM-013 | Proxy-Socket-Bind | S | Daemon lauscht auf UDS in eigenem Verzeichnis, Datei wird einzeln in die Sandbox gebunden, gRPC-Socket ist nie sichtbar. Escape-Test 2 grün |
| HUM-014 | CA-Verwaltung | M | CA pro Installation unter `$XDG_DATA_HOME`, Key 0600, `ca.crt` in Sandbox, Env-Kit (SSL_CERT_FILE, SSL_CERT_DIR, CURL_CA_BUNDLE, REQUESTS_CA_BUNDLE, NODE_EXTRA_CA_CERTS, DENO_CERT, GIT_SSL_CAINFO, CARGO_HTTP_CAINFO, PIP_CERT, NPM_CONFIG_CAFILE), Overlay `/etc/ssl/certs/ca-certificates.crt` mit CA |
| HUM-015 | MITM-Proxy-Kern | L | hudsucker-Wrapper: HTTP/1.1 + CONNECT, TLS-Terminierung mit rcgen-Leaf-Cache, Upstream via ALPN auf h1 gezwungen (h2 hinter `[experimental]`), Request-Body vollständig gepuffert (Cap 32 MB, darüber block), `Expect: 100-continue` erst nach Entscheidung, Response gestreamt |
| HUM-016 | Hold-Queue | M | `DashMap<FlowId, oneshot>` + Deadline-Timer, Timeout ⇒ `TimedOut` ⇒ 403 mit lesbarer Begründung an den Client, Zustandsautomat aus HUM-004 wird durchlaufen |
| HUM-017 | Konformitäts-Matrix | M | Integrationstests in-process mit axum-Fake-Upstream: curl h1, chunked, SSE, CONNECT, große Bodies, websocat-Upgrade (Passthrough), grpcurl (Trailer erhalten) |
| HUM-018 | gRPC-Server Grundgerüst | M | `GetInfo`, `Subscribe` (broadcast cap 1024, `Lagged`), `Decide` mit Allow/Block, Socket 0600, Session-Token in Metadata |
| HUM-019 | Flutter-Shell | M | Fenster, Icon-Rail, Command Palette, Statusleiste, Theme-Umschaltung, Verbindung zum Daemon/Fake, `GetInfo`-Versionscheck, Setup-Screen bei fehlendem Daemon |
| HUM-020 | Intercept-Screen v1 | L | Drei resizable Panes, Queue-Liste über `heldFlowsProvider`, Request-Karte mit Sektionen Query/Headers/Body (Raw), Aktionsleiste Allow/Block, Countdown-Ring, Ankunfts- und Verlassen-Animation, gegen Fake-Daemon |
| HUM-064 | CLI-Grundgerüst | M | Binary `humanitl` mit `clap`, Flags aus dem Config-Schema generiert, `sandbox run -- cmd`, `sandbox argv`, `sandbox check`, `daemon status`; Escape-Tests und Demo-Skripte nutzen die CLI statt Ad-hoc-Skripte |
| HUM-021 | Demo-Skript M1 | S | CI: `humanitl sandbox run -- curl https://example.com`, Request erscheint als `Held` im gRPC-Stream, Block ⇒ curl erhält 403. Escape-Test 3 grün |

### Sprint 2 — First Decision (M2)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-022 | Regel-Engine | L | YAML-Parser, Normalisierung (Punycode, lowercase, trailing dot), Label-Globs `*`/`**`, Method/Path/Scheme/Port, `expires` mit Session-Bindung, first-match-wins, Default ask, IP-Literale matchen nie. Escape-Test 4 komplett grün |
| HUM-023 | Host/SNI/Authority-Konsistenz | M | Nach TLS-Terminierung: `:authority`/`Host` == CONNECT-Ziel == SNI, sonst Block ohne Ask; kein Upstream-Coalescing über Authorities |
| HUM-024 | DNS erst nach Allow | M | Hold auf Hostname-String, Resolve nach Entscheidung, IP für die Verbindung gepinnt; Test mit Resolver-Statistik |
| HUM-025 | Findings-Detektoren Tier 1 | M | `Detector`-Trait, Registry, Regex+Checksum aus gitleaks-TOML (API-Keys, Tokens, JWT) plus E-Mail, IBAN (mod-97), Kreditkarte (Luhn), Telefon, IPv4; Nutzer-Terme aus Projekt-Settings (Kundennamen); Spans im `Analyzed`-Event |
| HUM-026 | Recorder | M | SQLite WAL, Migrationen, Schema aus 3.4, Bodies inline ≤ 256 KB sonst Blob-Store, Responses gestreamt gespiegelt, `ListFlows` mit Filter/Sort/Cursor serverseitig |
| HUM-027 | Rules-RPCs | S | list/add/update/remove/reorder/dry_run (welche vergangenen Flows hätten gematcht) |
| HUM-028 | Aktionsleiste komplett | M | Release-Valve-Pill, Dauer × Ziel-Raster, Regelsatz-Vorschau live (`allow · GET · *.npmjs.org · diese Session`), Default Session, Block nicht benachbart, Shortcuts aus Abschnitt 5, Inline-Bestätigung „Regel gespeichert · Rückgängig" |
| HUM-072 | Block mit Notiz | S | Optionales Notizfeld in der Aktionsleiste (aufklappbar, `N`), Text landet in `Decision::Block { reason: User, note }`, im 403-Body als `note:` und im Header `X-Humanitl-Note`, in History und Audit sichtbar; Länge auf 500 Zeichen begrenzt, kein Newline-Injection in Header |
| HUM-029 | Queue-Gruppierung und Batch | M | Gruppierung nach Host, Summary-Zeile mit Katalog-Identität und Findings-Zähler, „Allow 14 → host", Multi-Select, „+3 seit du liest", keine Umsortierung unter dem Cursor, Confirm nur bei Block-all > 5 |
| HUM-030 | Body-Ansichten | M | JSON-Tree (TreeView), Form-Felder, Raw (re_editor read-only), Hex für Binär, Findings inline unterstrichen (orange Secret, amber PII), große Bodies: Größe + erste 64 KB + Scan-Status |
| HUM-031 | Domain-Panel v1 | M | Katalog-Karte aus `catalog/domains.yaml` (~200 Dev-Services, Icon gebündelt, Kategorie, Beschreibung, „typisch für"), PSL-Apex hervorgehoben, Tranco-Rank-Badge aus gebündelter Liste, Unbekannt-Karte gestrichelt, Schnellregeln, kein automatischer Fetch |
| HUM-032 | History-Screen | L | TableView virtualisiert, Spalten aus Abschnitt 5, Filter-Syntax `host:… state:…`, Detail unten Request/Response Raw/Pretty, Doppelklick öffnet gehaltenen Flow im Intercept, Export HAR/JSONL/curl |
| HUM-033 | Rules-Screen | M | Geordnete Liste mit Drag-Reorder, Formular-Editor, „erstellt vor 2 min aus Request #41", Dry-Run-Panel, Bundled-Badge für Default-Regeln, Löschen mit Undo, Tabs „Gespeichert" und „Temporär" (Session-Regeln mit „dauerhaft machen" und Restlaufzeit) |
| HUM-065 | CLI rules/flows | S | `humanitl rules list|add|remove|test <url>`, `humanitl flows list|show <id>`, Ausgabe als Tabelle oder `--json`, gleiche Filter-Syntax wie History-Screen |
| HUM-034 | Notification und Tray | M | `flutter_local_notifications` mit Allow/Block-Aktionen bei Queue 0 → 1, `tray_manager`-Badge mit Zähler, Rückkehr-Banner „Agent wartet seit …", Fenster nach vorn bei Klick; GNOME-AppIndicator-Hinweis in Docs |
| HUM-035 | shadcn vs forui Entscheidung | S | Erledigt 2026-09-04 ohne den vorgesehenen Spike-Branch (Abweichung in CONVENTIONS 4.20). Das Ergebnis „weder noch" hat der Projekteigentümer am selben Tag zurückgenommen: `shadcn_flutter` wird aufgenommen (ADR-0009, Abschnitt „Revidiert am 2026-09-04 durch den Projekteigentümer") |
| HUM-036 | Demo-Skript M2 | S | CI e2e unter xvfb: Fake-Agent feuert 15 Requests, UI gruppiert, Batch-Allow mit Session-Regel, ein Block, ein Timeout, History zeigt alles, Export validiert |
| HUM-089 | acknowledge_findings wird nie gelesen | S | `DecideRequest.acknowledge_findings` entfernt, Nummer 6 und der Name mit Begründung reserviert, Vertragstabelle in `proto_contract.rs`, `proto_roundtrip.rs:156` und `flows.rs:339` nachgezogen, neuer Test `decide_request_reserves_the_never_read_acknowledge_findings_number`, `proto/descriptor.binpb` und `proto/generated.sha256` neu erzeugt, `docs/PROTOCOL.md` 4 führt die reservierten Nummern als Tabelle, HUM-049 in `backlog/sprint-4.md` zieht auf die freien Nummern 8 und 9 und bekommt eine Pfadliste, deren Dateien existieren; `PROTO_MAJOR`/`PROTO_MINOR` bleiben 1/2 und kein Verhalten ändert sich |
| HUM-091 | FlowSummary traegt keine registrierbare Domaene | M | `FlowSummary` bekommt `string apex = 25` (A-Label nach Public Suffix List, leer heißt unbekannt und wird nie geraten), gefüllt aus der Spalte `apex` auf dem History-Pfad und aus der `DomainTable` live; `humanitl flows list --json`, der Dart-`Flow` und beide Fakes tragen ihn, `apex:` filtert überall exakt gleich (`apex:github.io` trifft `a.b.github.io` nirgends), `app/lib/features/intercept/psl.dart` entfällt und Warteschlange wie Regel-Ziel „Domäne" arbeiten ohne zweiten `GetFlow`-Aufruf auf dem gelieferten Feld |
| HUM-094 | Katalogname und Editor fehlen dem M2-Demolauf | L | Katalog-Karte und Unbekannt-Karte ersetzen `DomainPanePlaceholder`, Gruppenkopf und Summary nennen den Dienst aus `catalog_id` („npm registry", „Looks like: npm install") statt des aus `psl.dart` geratenen Hosts, `app/assets/catalog/domains.yaml` ist byte-gleich mit `catalog/domains.yaml` (`make catalog-lint`), `M2_UI=1 tests/e2e/m2_first_decision/run.sh` endet 0 ohne `SKIPPED:`-Zeile mit mindestens 51 gezählten Behauptungen, davon zwei auf den Namen, und `BACKLOG.md:395` verspricht für M2 kein `Edit` mehr (Editor bleibt HUM-047, M4) |
| HUM-095 | Sitzungsregel aus dem Stapel traegt keine Herkunft | S | `humanitl flows decide <id> allow\|block --remember <PATTERN> [--remember-method\|-path\|-expires\|-note]` schickt einen `Decide` mit gefülltem `remember`, Aktion aus dem Verdikt, Ablauf `session` als Default und `created_from_flow_id` gleich der entschiedenen Id; ohne das Flag bleibt Aufruf und JSON unverändert; `--note` (an den Agenten) und `--remember-note` (in der Regel) kreuzen sich nicht; scheitert die Regel, wird nichts entschieden; `rules list --json` zeigt die Herkunft und der Regel-Bildschirm das Abzeichen; M2-Lauf legt die Sitzungsregel über die Entscheidung an statt über `rules add` |

### Sprint 3 — Agent Inside (M3)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-037 | AgentAdapter-Trait und OpenCode-Profil | M | Trait aus Abschnitt 6, Impl OpenCode: `opencode.json`-Template mit `@ai-sdk/openai-compatible` und baseURL, gebündelte `models.json` und `OPENCODE_MODELS_URL`, `OPENCODE_DISABLE_AUTOUPDATE=true`, `websearch: deny`, `webfetch: ask`, Env-Kit |
| HUM-071 | Agent-Briefing | S | `AgentAdapter::files` legt `~/.config/opencode/AGENTS.md` in der Sandbox an, Vorlage `agents/opencode/briefing.{en,de}.md` (~150 Token, Inhalt nach `docs/ARCHITECTURE.md` 8.1), Sprache aus `ui.language`; Test: Datei existiert in der Sandbox, Projektverzeichnis unverändert |
| HUM-073 | Meta-Endpoint `humanitl.internal` | M | Proxy beantwortet Requests an Host `humanitl.internal` selbst: `GET /` Kurzstatus (Session, Ask-Modus, gültige Regeln eine Zeile pro Regel, Timeout), `GET /why/<flow-id>` Entscheidung und Notiz, `POST /ask` Freitext bis 2 KB erzeugt `AgentAsk`-Event und Karte im UI mit Regelvorschlag; kein Upstream, kein DNS, Antworten `text/plain`; Tests für alle drei Pfade und für Ablehnung anderer Methoden |
| HUM-038 | Default-Regeln | S | `rules/default.yaml`: Block für models.dev, GitHub-Update-Check, Telemetrie-Hosts; Metrik im Test: gehaltene Requests vor dem ersten Prompt ≤ 1 |
| HUM-039 | LLM-Passthrough-Regel | M | Regel exakt Host+Port+Pfadpräfix (`/v1/`, `/api/`)+POST, gestreamt, komplett geloggt, Findings-Scan mit Warnung ohne Halt, eigener Zustand `passthroughLlm`, Setup-Feld mit „Test"-Button (listet Modelle) und dem einen erklärenden Satz |
| HUM-040 | Sandbox-Screen | M | Start/Stop, Status, Mounts (Projektordner-Picker, ro/rw, Satz „Der Agent sieht nur `/work` = …"), Env, Log-Tab, Stop-Modal bei laufendem Agent |
| HUM-041 | Isolation-Check-Panel und Ring | M | Daemon führt drei Prüfungen in der laufenden Sandbox aus, Panel mit drei grünen Zeilen in Klartext plus amber LLM-Ausnahme, Header-Ring mit drei Segmenten, „Exakte bwrap-Kommandozeile anzeigen", Fehlschlag deaktiviert Start mit konkretem Grund und „Diagnose kopieren", nie „trotzdem starten" |
| HUM-042 | Terminal | L | PTY lebt im Daemon, `Terminal`-Bidi-Stream chunked mit Resize, xterm2 in Flutter, OSC 52/8/Titel deaktiviert, Banner „Agent-Ausgabe ist nicht vertrauenswürdig", Zeile im Terminal wenn ein Request gehalten wird |
| HUM-043 | `/work`-Härtung | M | Nur Unterpfad, maskierte Pfade per tmpfs, Session-Ende: Diff-Zusammenfassung berührter Dateien, Secret-Scan über Diff, Symlinks nach außerhalb markiert; host-seitige Dateizugriffe mit `O_NOFOLLOW`. Escape-Test 5 (Dateisystem-Teil) grün |
| HUM-044 | Setup-Flow | M | Vier-Punkte-Checkliste (Daemon, LLM, Projekt, Sandbox-Check) statt leerer Queue, systemd-Unit-Installation per Klick, Coach-Mark am ersten gehaltenen Request („Angehalten, weil keine Regel passt") |
| HUM-045 | TLS-Fehler-Erkennung | S | Handshake-Abbruch aus der Sandbox wird als `Diagnostic` mit `FixAction::SetEnv` geliefert und als Karte gezeigt („curl hat das Humanitl-Zertifikat abgelehnt") mit „Fix kopieren" |
| HUM-066 | Profile | M | Profil-Datei bündelt Sandbox-Profil, Regelsatz, Agent, LLM-Endpoint, Timeout, Mounts, Env; global unter `$XDG_CONFIG_HOME/humanitl/profiles/` oder pro Projekt `.humanitl/profile.toml`, Projekt gewinnt; mitgelieferte Profile `default` und `llm-only`; Profil-Wahl im Setup und in der CLI |
| HUM-067 | `humanitl run` | L | Startet Daemon-Session im aktuellen Verzeichnis (`/work` = cwd), Profil per Flag oder `.humanitl/`, Agent aus Profil (Default OpenCode), Terminal ist das Terminal des Nutzers (PTY durchgereicht), `--ask terminal` mit Prompt-Format aus ADR-013, `--ask none` blockt alles ohne Regel; `llm-only` liefert die reine Inferenz-Instanz; UI kann sich an die laufende Session hängen und übernimmt die Queue |
| HUM-075 | `humanitl doctor` | M | Prüft bwrap-Version und Userns (`/proc/sys/kernel/unprivileged_userns_clone`, AppArmor-Profil auf Ubuntu 24.04+), seccomp-Fähigkeit, socat-frei (Shim), `$XDG_RUNTIME_DIR`, systemd user session, GTK/Impeller-Renderer, Tray-Unterstützung (AppIndicator), OpenCode im PATH, LLM-Erreichbarkeit; jede Zeile `ok|warn|fail` mit Diagnostic und Fix; läuft automatisch im Setup-Screen und per CLI mit `--json` |
| HUM-076 | LLM-Server finden | M | Button „LLM finden" im Setup und `humanitl llm discover`: host-seitig, nur auf Klick, TCP-Connect auf Ports 11434 (Ollama), 1234 (LM Studio), 8000 (vLLM), 8080 (llama.cpp) im lokalen /24 mit 200 ms Timeout, dann `GET /api/tags` bzw. `/v1/models`; Ergebnisliste mit Modellen, ein Klick setzt `llm.endpoint`; Warnhinweis, dass der Scan das LAN anspricht; Ergebnis wird nie automatisch übernommen |
| HUM-068 | Geführte Diagnostics im Sandbox-Screen | M | Jede Prüfung und jeder Startfehler zeigt Grund und Fix inline; LLM-Server unerreichbar ⇒ Diagnostic mit Ping-Ergebnis, Port, Vorschlag; Projektordner ohne Schreibrecht ⇒ Fix „als ro mounten"; bwrap zu alt ⇒ Paketbefehl kopierbar. Snapshot-Tests für alle Diagnostics-Codes |
| HUM-046 | Demo-Skript M3 | S | CI mit Ollama-Mock: OpenCode startet, erster Prompt, models.dev geblockt per Default-Regel, `webfetch` auf eine URL wird gehalten, Allow, Antwort im Terminal sichtbar; zweiter `webfetch` wird mit Notiz geblockt, Agent gibt die Notiz im Terminal wieder |
| HUM-087 | resolver.test_ca wirkt nicht | M | `humanitld --allow-test-ca` lädt die Wurzel aus `resolver.test_ca` als PEM und reicht sie an den Upstream-Stapel und die Endpunkt-Probe weiter; ohne Flag bleibt die Wurzelliste leer und der gesetzte Schlüssel erzeugt `CONFIG_008` statt einer nackten Log-Zeile, mit Flag und unbrauchbarer Datei startet der Daemon nicht (`CONFIG_007`, kein Socket); Paar-Test im Proxy zeigt mit `trust(root)` 200 und ohne 502 mit `reason: upstream_tls`; der M2-Lauf fährt über `https://`, Schritt 7 misst 200 statt 502 und das Ziel bedient 16 Anfragen; `docs/CONFIG.md`, `docs/DIAGNOSTICS.md` und die Schema-Fixture kommen aus den Generatorläufen, M1 bleibt grün |
| HUM-088 | experimental.upstream_port_map wirkt nicht | S | Der Schlüssel steht im Schema, wird validiert und hat im Proxy keinen Leser, während `docs/CONFIG.md` eine Portumlenkung zusagt; er wird entfernt statt eingebaut (Feld, Prüfung, Schema-Fixture, `docs/CONFIG.md`, alle Nennungen in `backlog/` und `tests/`), `resolver.overrides` übernimmt den Freiform-Tabellen-Fall in `precedence.rs`; Abnahme: `grep -rn "upstream_port_map" --include="*.rs" daemon/` ohne `target/` leer, eine `config.toml` mit dem Schlüssel scheitert mit `CONFIG_002` samt Schlüsselname, `cargo test -p humanitl-config` und `tests/e2e/m2_first_decision/run.sh` grün, CONVENTIONS 4.22 hält Grund und Rückkehrbedingung fest |

### Sprint 4 — Trusted Editor (M4)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-047 | Pseudonymisierungs-Editor | L | Split-View Original/Editiert, Findings-Rail gruppiert nach Typ, „Alle durch Pseudonyme ersetzen", per Finding Replace/Replace-all/Ignore/Ignore-always, manuelle Selektion `Ctrl+R`, Diff-Glow, Button „Editierte Version senden" mit Stift, Karte bekommt Chip „Edited" |
| HUM-048 | Pseudonym-Mapping | M | Stabil pro Session (`<EMAIL_1>`, `Client-A`), Mapping-Panel mit maskiertem Original, Speicherung getrennt und verschlüsselt (Keyring-Schlüssel), Secrets nur als Hash + Präfix, Export verschlüsselt |
| HUM-049 | Forward mit offenen Findings | S | Allow-Button wird amber „Senden mit 2 Findings", Vorab-Pause inline mit „Trotzdem senden / Pseudonymisieren / Blockieren", Setting: Checksum-verifizierte Secrets hart blocken |
| HUM-050 | Audit-Hash-Kette | M | JSONL append-only, kanonisches JSON, HMAC mit Installationsschlüssel, `verify`-Kommando, Head-Hash-Anchoring alle N Einträge mit UI-Anzeige, Tamper-Tests aus 4.5 komplett grün |
| HUM-051 | Audit-Screen | S | Kette verifizieren, Head-Hash, Export JSONL/CSV, Retention-Einstellung mit dokumentierter Löschung |
| HUM-052 | i18n DE/EN | M | ARB für alle Strings, Begriffe aus Abschnitt 5, Sprachwahl im Setup, Golden-Tests pro Sprache für Intercept-Karte |
| HUM-077 | Ein-Klick-Installation | M | Ein `.deb` enthält UI, Daemon, Shim, Profile; Erststart zeigt „Hintergrunddienst aktivieren" (installiert und startet die user unit per `FixAction::InstallService`, kein Terminal nötig); AppImage legt Unit und Binaries unter `~/.local` an und aktualisiert sie bei Versionswechsel; Deinstallation entfernt beides; `humanitl doctor` bestätigt |
| HUM-079 | Rücktausch von Pseudonymen in Text-Antworten | M | Nicht-gestreamte Antworten mit `text/*` oder `application/json` werden host-seitig nach `Responded` durch das Session-Mapping zurückübersetzt (`<EMAIL_1>` → Original), bevor sie an den Agenten gehen; gestreamte Antworten und Binärdaten bleiben unverändert und bekommen Header `X-Humanitl-Pseudonyms: untranslated`; History zeigt beide Fassungen; Setting `pseudonyms.translate_responses` (Default true) |
| HUM-053 | Packaging | M | `.deb` und AppImage via fastforge, Daemon neben Bundle, systemd user unit mit Härtung aus 4.4, Erststart bietet Installation an, Wayland-Test auf NVIDIA und Intel |
| HUM-069 | Settings-Screen mit Progressive Disclosure | L | Generiert aus dem Config-Schema (HUM-062): Gruppen, Suche über alle Stufen, `basic` im Setup, `advanced` sichtbar, `expert` eingeklappt mit Warnhinweis bei Sicherheitsrelevanz; jedes Feld mit Beschreibung, Default, Reset, Herkunft (global/Projekt/CLI/Env); „Config-Datei öffnen" und Live-Reload; Netzwerkpfad zum LLM-Server, Agent-Wahl, Sandbox-Profil, Mounts, Env, Timeouts, Detektoren, Katalogquellen alle erreichbar |
| HUM-078 | Paritäts-Tabelle und CI-Check | S | `cargo xtask docs` erzeugt `docs/reference/parity.md` aus der Proto (Service-Methoden), der clap-Struktur (Subkommandos mit Attribut `#[humanitl(rpc = "…")]`) und einer UI-Registry (`app/lib/core/parity.dart`, Liste RPC → Screen); CI-Job `parity-check` schlägt fehl bei RPC ohne CLI-Zeile; UI-Lücken werden als `warn` gelistet |
| HUM-070 | CLI config/audit/daemon | S | `humanitl config get|set|schema|edit`, `audit verify|export`, `daemon install|status|logs`; `set` validiert gegen Schema und zeigt Diagnostic bei Fehler |
| HUM-054 | Golden- und Widget-Tests | M | alchemist im CI-Modus für Queue-Zeile (drei Zustände), Karte, Aktionsleiste, Domain-Panel bekannt/unbekannt, Isolation-Panel |
| HUM-055 | Demo-Skript M4 | S | e2e: Request mit E-Mail und Kundenname, Editor öffnet, „Alle ersetzen", Senden, History zeigt Edited, Audit verifiziert, Export prüft Pseudonyme |
| HUM-090 | Paritaetsluecke zwischen CLI, RPC und UI | S | `humanitl flows decide` nimmt weitere Flows über wiederholbares `--also ID` und legt mit `--remember --remember-host PATTERN` (Aktion aus dem Verdikt, Default `--remember-expires session`) die Regel in derselben `Decide`-Anfrage an, mit `created_from_flow_id` der ersten Id; Textausgabe eine Zeile je Flow plus `remembered …`, `--json` liefert `results[]` und `created_rule`, Exit 1 sobald ein genannter Flow abgelehnt wurde, doppelte Id ist `CLI_004` vor dem Aufruf; M2-Schritt 2 gibt die zwölf npm-Anfragen in einem Aufruf frei und prüft die Herkunft der Regel; Proto, Daemon, Fake und `app/` bleiben unberührt, eine Prüfung auf Feld-Ebene bleibt ausdrücklich offen |
| HUM-092 | Export ist Fachlogik in der Anwendung | L | `ExportFlows`-RPC im Daemon kodiert HAR, JSONL, CSV und curl aus dem Recorder; `app/lib/features/history/export/` entfällt und `grep` findet die vier Encoder-Namen nicht mehr unter `app/lib`; Rust-Encoder sind byte-identisch zu den Fixtures der Dart-Tests, Oberfläche und `humanitl flows export` liefern für dieselbe Auswahl dieselbe Datei (`cmp` endet mit 0); `scripts/ci/check-client-logic.sh` wird bei einem Wiedereinbau rot; `backlog/sprint-2.md` (Zeile 583, HUM-032) und `CONVENTIONS.md` 4.18 korrigiert, damit die Anordnung „in der UI" nicht zurückkommt |

### Sprint 5 — Härtung und Release (M5)

| ID | Titel | Größe | Akzeptanzkriterien |
|---|---|---|---|
| HUM-056 | Fuzzing | M | cargo-fuzz-Targets für Request-Parser-Pfad, Decoder (gzip, brotli, chunked), Regel-Parser; 10 Minuten pro Target in Nightly-CI |
| HUM-057 | Ressourcen-Limits | S | Dekompressions-Ratio-Limit, Body-Caps für Vorschau (8 MB), Timeouts, Backpressure-Test mit 1000 Flows in 10 s |
| HUM-058 | Fehlerpfade im UI | M | Daemon weg ⇒ Setup-Screen, keine veralteten Daten als live; Sandbox-Start-Fehler inline; Timeout-Karte mit Retry-Hinweis; WebSocket-Upgrade-Karte mit ausgesprochener Einschränkung; Streaming-Response live |
| HUM-059 | Dokumentation | M | README (Installation, drei Garantien, Screenshot), SECURITY.md final, THREAT-MODEL.md final, DESIGN.md, Regel-Referenz, Agent-Profile schreiben |
| HUM-086 | Repository auf Englisch | M | Alle Dokumente, Kommentare, ADRs, Backlog-Dateien, Diagnostics-Texte und Commit-Vorlagen ins Englische übersetzen; Deutsch bleibt nur in `app_de.arb`; `CLAUDE.md`, `CONTRIBUTING.md`, `AGENTS.md` auf English-only umstellen; Lint `scripts/ci/lint-docs.sh` prüft, dass keine deutschen Stoppwörter in Doku und Kommentaren stehen; erst nach HUM-059, vor HUM-060 |
| HUM-060 | Release 0.1 | S | Tag `v0.1.0` bzw. Snapshot-Tag `v0.1.0-snapshot.N` per `git push origin <tag>`, tag-getriggerte Release-Action baut das versionierte `.deb` (Version aus dem Tag) und AppImage, prüft Signatur und Checksummen, hängt beides als GitHub Pre-Release an; Abnahme: frisches Debian, `.deb` aus dem Release installieren, App startet, Demo-Skripte M1–M4 grün |
| HUM-061 | Puffer | L | Reserve für MITM-Randfälle, Wayland-Themen, Flutter-Anhebungen |
| HUM-093 | M1-Demolauf raeumt sein Verzeichnis nicht weg | S | `m1_sealed_box.sh` löscht seinen Wegwerf-Baum im vorhandenen `collect()` über den neuen Helfer `e2e_drop_workdir` in `lib.sh` (gesetzt, vorhanden, Präfix `/tmp/hum-e2e-`), den auch M2 nutzt; nach grünem wie rotem Lauf bleibt kein neues `/tmp/hum-e2e-*` zurück, `target/e2e` enthält unverändert dieselben Artefakte, und `E2E_KEEP_WORKDIR=1` behält den Baum und nennt seinen Pfad im Protokoll |

---

## 9. Nach dem MVP (priorisiert)

1. **M6 Docker-Backend + HTTP/2-Upstream.** Zweite `SandboxBackend`-Impl (`--network none` + UDS + eigenes seccomp-JSON), Image-Layering `humanitl/base → agent-opencode → packs`, Cache-Volumes pro Projekt, UID-Mapping. h2 zum Upstream aus `[experimental]` heben.
2. **M7 Browser.** Issues: HUM-080 Sandbox-Profil `browser` (Chromium-Flags, SPKI-Hash der CA, `AF_UNIX` erlaubt, `humanitl_browser.py` im Pack, Briefing-Zusatz); HUM-081 Reverse-Bridge im Shim und `cdp.sock`; HUM-082 CDP-Client im Daemon (`chromiumoxide`, Screencast, Input, Tab-Liste); HUM-083 gRPC `Browser`-Bidi-Stream (Frame JPEG + Metadaten, Input-Events, Takeover-Flag); HUM-084 Browser-Tab im UI (Frame als `Image.memory`, Klick- und Tastatur-Weiterleitung, Übernahme-Modus mit sichtbarem Rahmen, Tab-Wechsel, „zurück an den Agenten"); HUM-085 Screenshot-Vorschau für das Domain-Panel über denselben Browser (ersetzt M10-Teil). Fallstricke: Chromium braucht `--no-sandbox` in bwrap; Screencast-Frames drosseln (max 10 fps, JPEG 60); Tastatur-Layouts in `dispatchKeyEvent`; nodriver-Versionen pinnen.
3. **M8 Plugin-API.** Externe Detektoren und Aktionen über gRPC, WASM-Detektoren, Nutzer-Katalog, Panel-Slots. Erst wenn drei echte Bedarfe dokumentiert sind.
4. **M9 Response-Moderation.** Rücktausch auch in gestreamten Antworten (Token-Grenzen über Chunk-Puffer), optional Responses halten (`ask` auf Response-Ebene). Der einfache Rücktausch für nicht-gestreamte Text-Antworten ist bereits MVP (HUM-079).
5. **M10 Domain-Vorschau Live.** Nutzer-getriggerter Favicon/og:title-Fetch host-seitig mit Limits; Screenshot via Playwright in eigener netzwerkbeschränkter Sandbox, als Bild ins UI.
6. **M11 Credential-Injection.** Echte Tokens (GitHub, Registries) nur im Proxy, Sandbox sieht Platzhalter; Push nur auf konfigurierten Branch.
7. **OpenCode-Permission-Bridge.** `opencode serve` SSE + `POST /session/:id/permissions/:id`, OpenCodes eigene Tool-Prompts nativ in Flutter rendern. Weitere Agent-Adapter: Aider, Codex (`--oss`), Claude Code (via `ANTHROPIC_BASE_URL`).
8. **WebSocket-Frame-Hold.** Frames anhalten statt nur aufzeichnen.
9. **M12 macOS.** Seatbelt-Backend (Muster sandbox-runtime), gleicher Daemon, gleiche Proto.
10. **M13 microVM.** Firecracker/Kata oder Docker Sandboxes mit vsock-only, für stärkeres Threat Model.
11. **Upstream-Proxy und Tor.** `Egress`-Adapter `HttpProxy` und `Socks5h` (`tokio-socks`), Config `egress.via`, Regel-Feld `via`, Tor-Check im Isolation-Panel, Leak-Test: kein lokaler DNS-Lookup im Tor-Modus. Nice-to-have.
12. **Flatpak.** UI im Flatpak, Daemon außerhalb.
13. **OpenTelemetry** hinter Cargo-Feature, Team-Kataloge, Regel-Profile teilen.

---

## 10. Risiken und Gegenmaßnahmen

| Risiko | Gegenmaßnahme, wann |
|---|---|
| Sandbox-Escape über einen nicht bedachten Pfad (vererbte FDs, `/proc/net`, io_uring) | Escape-Tests vor dem Proxy schreiben (Sprint 0/1), FD-Enumeration beim Agent-Start, externes Review der bwrap-Zeile vor 0.1 |
| MITM-Randfälle blockieren die Demo (h2 ALPN, chunked, SSE durch Bun-fetch) | Konformitäts-Matrix Sprint 1, h1-Upstream erzwingen, h2 hinter Flag |
| OpenCode telefoniert beim Start nach Hause und flutet die Queue | Default-Regeln + Metrik „≤ 1 gehaltener Request vor erstem Prompt" (Sprint 3) |
| Eine Komponentenbibliothek vor 1.0 bricht jede Release | Entschärft: HUM-035 nimmt am 2026-09-04 keine auf. `packages/ui` steht auf reinem Flutter, der Wrapper bleibt als Naht für eine spätere Bibliothek |
| Solo-Scope-Creep (Katalog, Docker, Plugins) | Abschnitt 9 ist die Grenze, Demo-Skript grün ist Merge-Bedingung |
| Nutzer klickt unter Last „Allow all" | Gruppierung, Katalog-Identität, Batch nur pro Host, Confirm bei Block-all |
| LLM-Host ist geteilt und ungeschützt (Ollama ohne Auth) | Setup-Satz „Nur eine Maschine, die du kontrollierst", Findings-Warnung im Passthrough, Threat Model dokumentiert |
| Wayland/NVIDIA-Rendering mit Impeller | Früh testen (Sprint 1), `--no-enable-impeller` als Fallback dokumentieren |

---

## 11. Quellen (Auswahl)

Isolation: anthropic-experimental/sandbox-runtime (github.com/anthropic-experimental/sandbox-runtime), Claude Code Sandboxing (code.claude.com/docs/en/sandboxing), OpenAI Codex linux-sandbox README (github.com/openai/codex/blob/main/codex-rs/linux-sandbox/README.md), bubblewrap Manpage (manpages.debian.org/trixie/bubblewrap), Docker Sandboxes (docs.docker.com/ai/sandboxes), Landlock (man7.org/linux/man-pages/man7/landlock.7.html), Moby internal-network DNS advisory GHSA-mq39-4gv4-mvpx.

Proxy: hudsucker (github.com/omjadas/hudsucker), mitmproxy Flow-API und Modes (docs.mitmproxy.org), rama (github.com/plabayo/rama), coder/boundary (github.com/coder/boundary), pipelock (github.com/luckyPipewrench/pipelock), mockttp (github.com/httptoolkit/mockttp), ZAP Break API (zaproxy.org/docs/api), Burp Scope (portswigger.net/burp/documentation/desktop/tools/target/scope), gitleaks (github.com/gitleaks/gitleaks), ripsecrets (github.com/sirwart/ripsecrets), Presidio (presidio.dataprivacystack.org), Dart `redact` (pub.dev/packages/redact).

Agent: OpenCode Providers/Tools/Permissions/Server/Network (opencode.ai/docs), Ollama-Integration (docs.ollama.com/integrations/opencode), Claude Code Network Config (code.claude.com/docs/en/network-config), Aider Ollama (aider.chat/docs/llms/ollama.html), Codex Config (developers.openai.com/codex/config-basic).

UX: Burp Intercept, ZAP Breakpoints, Caido Intercept, Charles/Proxyman Breakpoints, HTTP Toolkit Rewriting, OpenSnitch Pop-ups (github.com/evilsocket/opensnitch/wiki), Little Snitch Alert (help.obdev.at), Tranco (tranco-list.eu), Public Suffix List (publicsuffix.org).

Flutter: Flutter 3.47 Release Notes (docs.flutter.dev/release), shadcn_flutter (pub.dev/packages/shadcn_flutter), forui, two_dimensional_scrollables, riverpod 3, grpc-dart UDS (github.com/grpc/grpc-dart/issues/299), re_editor, xterm2, flutter_pty, window_manager, tray_manager, flutter_local_notifications_linux, fastforge (github.com/fastforgedev/fastforge), flatpak-flutter (github.com/TheAppgineer/flatpak-flutter), Flathub-Diskussion zu bwrap im Flatpak (discourse.flathub.org/t/3572), alchemist.
