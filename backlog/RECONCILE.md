# Reconciliation items (from sprint forks)

## From sprint-0
1. SANDBOX_006 new code -> add to registry (HUM-063 codes.rs)
2. FixAction::AddRule(Rule) needs Rule in core -> move Rule/Matcher/Action/Expiry to humanitl-core; rules crate = matching only. Update CONVENTIONS 3.2/3.3.
3. Proto extended: GetConfig/SetConfig, Diagnostic as FlowEvent variant, DecideRequest.remember, RulesRequest.make_permanent, Sandbox.argv -> update BACKLOG 3.3 + CONVENTIONS 3.6
4. FlowState::on signature: HUM-004 uses Transition input, returns (FlowState, FlowEvent) -> adopt in CONVENTIONS 3.2 (event is output)
5. ip:/cidr: matching in HUM-022 (ok)
6. Fake daemon in Rust (`humanitld --fake`), Dart FakeDaemonClient for widget tests -> update BACKLOG HUM-005 row
7. Escape test filenames esc-N-name.sh vs esc-N.sh -> pick `esc-N-<name>.sh`, update CONVENTIONS 3.11

## From sprint-5
1. limits.* group (HUM-057) with aliases for hold.body_cap_bytes, preview.cap_bytes, ipc.event_buffer -> add Config.limits to CONVENTIONS 3.7, mark old as alias
2. BlockReason += HoldMemory, HoldMaxFlows, ClientTimeout; Held event += queue_bytes, queue_count -> CONVENTIONS 3.2/3.6
3. Diagnostic code register: reserve DAEMON_001..004, SANDBOX_001..012, TLS_001..003, LLM_001..004, RULES_001, TERM_001, RECORDER_001, LIMIT_001..006 in codes.rs (HUM-063)
4. HModal wrapper in packages/ui -> add to CONVENTIONS 3.9 + HUM-008
5. --dart-define=FAKE=<scenario> vs --fake -> decide: Flutter `--dart-define=HUMANITL_FAKE=<scenario>` (Flutter has no custom CLI flags); daemon `humanitld --fake <file>`
6. daemon/xtask crate -> add to CONVENTIONS 3.1 (outside dep rules)
7. seccomp.rs filename in shim -> HUM-012 must use `daemon/bin/humanitl-shim/src/seccomp.rs`

## From my own changes (post-fork)
- seccomp: allow AF_INET/AF_INET6; AF_UNIX per profile (browser). Sprint-1 HUM-012 + sprint-0 HUM-006 ESC-1 must reflect: AF_INET socket() to loopback SUCCEEDS; AF_UNIX/NETLINK/PACKET EPERM.
- bridges list in profile (HUM-010, HUM-012)
- Egress port (HUM-015): no TcpStream::connect outside Egress
- New issues to add to sprint files: HUM-074 (sprint 0), HUM-072 (sprint 2), HUM-071, HUM-073 (sprint 3)

## Status 2026-09-02 19:50
- Alle sechs sprint-N.md vollständig (Forks starben erst beim Abschlussbericht, Sprint 1/2/3/4 haben ihre Inkonsistenz-Listen nicht mehr gemeldet).
- Offen: (1) Sprint-1..4 per grep gegen CONVENTIONS abgleichen (seccomp-Familien, Bridges-Liste, Egress-Port, Rule in core, Transition-Signatur, Config.limits, Diagnostic-Register, HModal, HUMANITL_FAKE, xtask). (2) Neue Issues in Sprint-Files nachtragen: HUM-074 (S0), HUM-072 (S2), HUM-071 + HUM-073 (S3). (3) Codex + Antigravity read-only Review über BACKLOG.md, ARCHITECTURE.md, CONVENTIONS.md. (4) Feedback prüfen, einarbeiten.

## From sprint-2 (reported after limit)
1. rules YAML: `match.upgrade: websocket` im Schema und RuleMatch-Proto -> CONVENTIONS 3.3 nachziehen
2. Blob-Pfad sharded `blobs/<hex[0..2]>/<hex>` -> CONVENTIONS 3.4 anpassen (übernehmen)
3. Neue Config-Schlüssel: resolver.*, upstream.connect_timeout_secs, experimental.upstream_port_map, findings.*, recorder.max_body_bytes -> CONVENTIONS 3.7 (mit Config.limits aus Sprint 5 zusammenführen)
4. Proto: DomainInfo in Received, RulesChanged-Event, DecideRequest.remember/DecideResponse.created_rule, eigene rules.proto -> CONVENTIONS 3.6 + BACKLOG 3.3
5. CLI-Exit-Codes 10 (block), 11 (ask) für `rules test` -> CONVENTIONS 3.8
6. Session-Regeln vor persistenten in RulesStore::effective() -> in ADR-007 festhalten
7. HostPattern nach core verschieben (catalog braucht es) -> passt zu "Rule nach core" aus Sprint 0
8. Sprint-2 kennt Egress-Port noch nicht (PinnedConnector/Resolver) -> HUM-024 auf Egress::Direct umformulieren

## Status 2026-09-02 (Abschluss)
- Alle Punkte oben in CONVENTIONS.md Abschnitt 4 aufgelöst und in die Sprint-Files (Abgleich-Hinweise oben, Textkorrekturen) übertragen.
- Reviews Antigravity (10 Punkte) und Codex (10 Punkte) eingearbeitet, siehe CONVENTIONS.md 4.10 und BACKLOG.md (ADR-001, 005, 006, 007, 008, 013, 4.1, 4.5, M1, Schätzung).
- Neue Issues: HUM-071..079. Gesamt 79.
- Offen: nichts Blockierendes. Nächster Schritt: Commit, dann Sprint 0 (HUM-001).
