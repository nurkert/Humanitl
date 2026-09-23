//! Vom Ereignisstrom ins Audit-Log (HUM-050). Nur Verdrahtung.
//!
//! Der Sink hört denselben Ereignisstrom wie Oberfläche und Aufzeichnung und
//! schickt für jeden Vorgang, der ins Log gehört, einen Record an den
//! Schreiber. Was in einem Record steht, entscheiden die Konstruktoren in
//! `humanitl_audit::kinds`: Der Sink reicht ihnen die Typen des Kerns, und sie
//! nehmen sich heraus, was ins Log darf. Bodies, Header, der Pfad im Klartext
//! und die Notiz einer Blockierung kommen dort nie an.
//!
//! | Ereignis | Record |
//! |---|---|
//! | Start des Sinks | `session.started` |
//! | `Received`, dann `Analyzed` | `flow.received`, sobald die Funde feststehen, spätestens mit der Entscheidung |
//! | `Decided`, `TimedOut` | `flow.decided` mit `unresolved_findings` und `acknowledged` (HUM-160); bei einem Block des Daemons zusätzlich `flow.blocked_reason` |
//! | `Forwarded`, `ResponseHeaders`, `ResponseChunk`, `Recorded` | `flow.responded` |
//! | Ende des Sinks | `session.ended` mit den Zahlen der Sitzung |
//!
//! `daemon.started` und `daemon.stopped` schreibt `main`, `rule.*` und
//! `llm.discover` der gRPC-Dienst. Ohne Quelle im heutigen Code bleiben
//! `flow.forwarded` (das Ereignis nennt die festgenagelte Zieladresse nicht),
//! `isolation.check` (die Messung läuft im Sandbox-Dienst und geht nur an den
//! Client, der gefragt hat), `config.changed` (es gibt kein `SetConfig`,
//! HUM-069), `pseudonym.created` und `finding.allowlisted` (weder Editor noch
//! Ausnahmeliste melden etwas, HUM-045 ff.) und `audit.verified` (die Prüfung
//! über RPC baut HUM-070).
//!
//! Verpasst der Sink Ereignisse, weil er hinter dem Rundfunk zurückbleibt,
//! fehlen sie im Log; `tracing` sagt, wie viele. Das ist die dokumentierte
//! Grenze „nie geschriebene Ereignisse" (`docs/SECURITY.md`, Abschnitt 8).

use std::collections::HashMap;
use std::time::SystemTime;

use humanitl_audit::kinds::{
    DecisionKind, FlowBlockedReason, FlowDecided, FlowReceived, FlowResponded, SessionEnded,
    SessionStarted,
};
use humanitl_audit::{AuditHandle, RecordKind};
use humanitl_core::{DecidedFindings, Decision, DecisionSource, FlowEvent, FlowId, SessionId};
use tokio::sync::broadcast::error::{RecvError, TryRecvError};
use tokio::sync::{broadcast, oneshot};

/// Der laufende Sink einer Sitzung.
pub(crate) struct AuditSink {
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<SessionEnded>,
}

impl AuditSink {
    /// Schreibt `session.started` und hört ab jetzt den Strom `events`.
    ///
    /// `events` muss abonniert sein, bevor der erste Flow kommt; was vorher
    /// geschah, sieht ein Rundfunk-Empfänger nicht.
    pub(crate) fn start(
        audit: AuditHandle,
        session: SessionId,
        started: SessionStarted,
        events: broadcast::Receiver<FlowEvent>,
    ) -> Self {
        audit.record(Some(session), RecordKind::SessionStarted(started));
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(run(Tally::new(audit, session), events, stopped));
        Self { stop, task }
    }

    /// Nimmt, was schon im Strom liegt, schreibt `session.ended` und endet.
    pub(crate) async fn finish(self) -> SessionEnded {
        let _ = self.stop.send(());
        self.task.await.unwrap_or_default()
    }
}

async fn run(
    mut tally: Tally,
    mut events: broadcast::Receiver<FlowEvent>,
    mut stopped: oneshot::Receiver<()>,
) -> SessionEnded {
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) => tally.apply(&event),
                Err(RecvError::Lagged(n)) => lagged(n),
                Err(RecvError::Closed) => break,
            },
            _ = &mut stopped => {
                loop {
                    match events.try_recv() {
                        Ok(event) => tally.apply(&event),
                        Err(TryRecvError::Lagged(n)) => lagged(n),
                        Err(_) => break,
                    }
                }
                break;
            }
        }
    }
    tally.finish()
}

fn lagged(n: u64) {
    tracing::warn!(
        dropped = n,
        "the audit sink fell behind the event stream; these events are not in the audit log"
    );
}

/// Was der Sink je Flow wissen muss, bis der Flow aufgezeichnet ist.
#[derive(Default)]
struct Pending {
    /// Der Record des Eintreffens, solange die Funde noch fehlen.
    received: Option<FlowReceived>,
    /// Wie zuletzt entschieden wurde, sobald entschieden ist.
    decided: Option<DecisionKind>,
    forwarded_at: Option<SystemTime>,
    status: Option<u16>,
    response_bytes: u64,
}

/// Übersetzt Ereignisse in Records und zählt die Sitzung mit.
pub(crate) struct Tally {
    audit: AuditHandle,
    session: SessionId,
    flows: HashMap<FlowId, Pending>,
    counts: SessionEnded,
}

impl Tally {
    pub(crate) fn new(audit: AuditHandle, session: SessionId) -> Self {
        Self {
            audit,
            session,
            flows: HashMap::new(),
            counts: SessionEnded::default(),
        }
    }

    fn record(&self, kind: RecordKind) {
        self.audit.record(Some(self.session), kind);
    }

    /// Schreibt den Record des Eintreffens, falls er noch aussteht.
    fn flush_received(&mut self, flow: FlowId) {
        let received = self
            .flows
            .get_mut(&flow)
            .and_then(|pending| pending.received.take());
        if let Some(received) = received {
            self.record(RecordKind::FlowReceived(received));
        }
    }

    pub(crate) fn apply(&mut self, event: &FlowEvent) {
        match event {
            FlowEvent::Received {
                flow_id, request, ..
            } => {
                self.counts.flows_total += 1;
                self.flows.insert(
                    *flow_id,
                    Pending {
                        received: Some(FlowReceived::new(*flow_id, request, &[])),
                        ..Pending::default()
                    },
                );
            }
            FlowEvent::Analyzed {
                flow_id, findings, ..
            } => {
                let received = self
                    .flows
                    .get_mut(flow_id)
                    .and_then(|pending| pending.received.take());
                if let Some(received) = received {
                    self.record(RecordKind::FlowReceived(received.with_findings(findings)));
                }
            }
            FlowEvent::Held { flow_id, .. } => {
                self.counts.held += 1;
                self.flush_received(*flow_id);
            }
            FlowEvent::Decided {
                flow_id,
                decision,
                source,
                findings,
                ..
            } => self.decided(*flow_id, decision, *source, findings),
            // Der Ablauf der Frist kommt als eigenes Ereignis, nicht als
            // `Decided` (`FlowState::on`, Übergang `Timeout`).
            FlowEvent::TimedOut { flow_id, .. } => {
                self.decided(
                    *flow_id,
                    &Decision::TimedOut,
                    DecisionSource::Timeout,
                    &DecidedFindings::default(),
                );
            }
            FlowEvent::Forwarded { flow_id, at } => {
                self.flows.entry(*flow_id).or_default().forwarded_at = Some(*at);
            }
            FlowEvent::ResponseHeaders {
                flow_id, status, ..
            } => {
                self.flows.entry(*flow_id).or_default().status = Some(*status);
            }
            FlowEvent::ResponseChunk { flow_id, len, .. } => {
                let pending = self.flows.entry(*flow_id).or_default();
                pending.response_bytes = pending.response_bytes.saturating_add(*len);
            }
            FlowEvent::Recorded { flow_id, at } => self.recorded(*flow_id, *at),
            FlowEvent::Failed { .. }
            | FlowEvent::Lagged { .. }
            | FlowEvent::Diagnostic { .. }
            | FlowEvent::AgentAsk { .. } => {}
        }
    }

    fn decided(
        &mut self,
        flow: FlowId,
        decision: &Decision,
        source: DecisionSource,
        findings: &DecidedFindings,
    ) {
        self.flush_received(flow);
        let kind = DecisionKind::of(decision, source);
        let pending = self.flows.entry(flow).or_default();
        match pending.decided {
            None => {}
            // Das System nimmt eine Freigabe zurück, bevor etwas hinausging
            // (abgelehnter Edit, bestätigtes Geheimnis in der bearbeiteten
            // Fassung). Das ist eine zweite Entscheidung und kein Echo: Sie
            // steht als eigener `flow.decided` im Log, und die Sitzung zählt
            // den Flow danach als Block statt als Freigabe (HUM-160).
            Some(before) if is_retraction(before, decision, source) => {
                *counter(&mut self.counts, before) =
                    counter(&mut self.counts, before).saturating_sub(1);
            }
            // Jedes andere zweite Wort über denselben Flow zählt nicht noch
            // einmal, etwa `Decided(TimedOut)` nach `TimedOut`.
            Some(_) => return,
        }
        pending.decided = Some(kind);
        *counter(&mut self.counts, kind) += 1;
        // Mit der Spur der Funde (HUM-160): wie viele offen hinausgingen und
        // wie viele davon der Mensch bestätigt hat.
        self.record(RecordKind::FlowDecided(
            FlowDecided::new(flow, decision, source).with_findings(findings),
        ));
        if let Some(reason) = FlowBlockedReason::of(flow, decision, source) {
            self.record(RecordKind::FlowBlockedReason(reason));
        }
    }

    fn recorded(&mut self, flow: FlowId, at: SystemTime) {
        self.flush_received(flow);
        let Some(pending) = self.flows.remove(&flow) else {
            return;
        };
        let Some(status) = pending.status else {
            return;
        };
        let duration_ms = pending
            .forwarded_at
            .and_then(|forwarded| at.duration_since(forwarded).ok())
            .map_or(0, |took| {
                u64::try_from(took.as_millis()).unwrap_or(u64::MAX)
            });
        self.record(RecordKind::FlowResponded(FlowResponded {
            flow: flow.to_string(),
            status,
            size: pending.response_bytes,
            duration_ms,
            // Der Proxy streamt jede Antwort durch, er hält keine an
            // (`docs/SECURITY.md` 10, Punkt 4). Ein gepufferter Weg entsteht
            // erst mit dem Rücktausch von Pseudonymen (HUM-079).
            streamed: true,
        }));
    }

    /// Schreibt, was noch aussteht, und `session.ended`.
    pub(crate) fn finish(mut self) -> SessionEnded {
        let open: Vec<FlowId> = self.flows.keys().copied().collect();
        for flow in open {
            self.flush_received(flow);
        }
        self.record(RecordKind::SessionEnded(self.counts));
        self.counts
    }
}

/// Der Zähler der Sitzung, unter dem eine Entscheidung steht.
fn counter(counts: &mut SessionEnded, kind: DecisionKind) -> &mut u64 {
    match kind {
        DecisionKind::Allow => &mut counts.allowed,
        DecisionKind::AllowEdited => &mut counts.allowed_edited,
        DecisionKind::Block => &mut counts.blocked,
        DecisionKind::TimedOut => &mut counts.timed_out,
        DecisionKind::AutoAllow | DecisionKind::AutoBlock => &mut counts.auto_rule,
        DecisionKind::Passthrough => &mut counts.passthrough,
    }
}

/// Wahr, wenn das System eine Freigabe zurücknimmt: auf eine Entscheidung,
/// die hinausließe, folgt ein Block durch `System`. Der Automat erlaubt genau
/// diesen zweiten Übergang (`FlowState::on`, `Decided(Allow|AllowEdited)` nach
/// `Decided(Block)`), und nur ihn.
fn is_retraction(before: DecisionKind, decision: &Decision, source: DecisionSource) -> bool {
    let let_out = matches!(
        before,
        DecisionKind::Allow
            | DecisionKind::AllowEdited
            | DecisionKind::AutoAllow
            | DecisionKind::Passthrough
    );
    let_out && matches!(decision, Decision::Block { .. }) && source == DecisionSource::System
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, SystemTime};

    use humanitl_audit::{AuditKey, AuditRecord, AuditWriter, KeyOrigin, WriterOptions};
    use humanitl_core::{
        Authority, BlockReason, BodyRef, Decision, DecisionSource, Finding, FindingKind,
        FindingLocation, FlowEvent, FlowId, HostName, HttpRequest, Method, RuleId, Scheme,
        SessionId, Tier,
    };

    use super::Tally;

    fn log() -> (tempfile::TempDir, AuditWriter) {
        let dir = tempfile::tempdir().unwrap();
        let key = AuditKey::from_bytes([5; 32], KeyOrigin::File);
        let (writer, _) = AuditWriter::open(
            &dir.path().join("audit.jsonl"),
            &key,
            WriterOptions::default(),
            &[],
            None,
        )
        .unwrap();
        (dir, writer)
    }

    fn records(writer: AuditWriter) -> (String, Vec<AuditRecord>) {
        writer.handle().sync().unwrap();
        let text = std::fs::read_to_string(writer.path()).unwrap();
        drop(writer);
        let records = text
            .lines()
            .map(|line| AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect();
        (text, records)
    }

    fn received(flow: FlowId, path: &str, body: &'static [u8]) -> FlowEvent {
        let mut request = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
            path.to_owned(),
        );
        request.body = BodyRef::from_bytes(bytes::Bytes::from_static(body));
        FlowEvent::Received {
            flow_id: flow,
            at: SystemTime::now(),
            request: Box::new(request),
        }
    }

    fn email() -> Finding {
        Finding {
            kind: FindingKind::Email,
            span: 9..26,
            location: FindingLocation::Body,
            tier: Tier::Regex,
            value_hash: [0; 32],
            display_prefix: "alice…".to_owned(),
        }
    }

    #[test]
    fn a_held_and_blocked_flow_becomes_received_and_decided_without_payload() {
        let (_dir, writer) = log();
        let mut tally = Tally::new(writer.handle(), SessionId::new());
        let flow = FlowId::new();
        tally.apply(&received(
            flow,
            "/submit?token=quux-geheim",
            b"{\"mail\":\"alice@example.org\"}",
        ));
        tally.apply(&FlowEvent::Analyzed {
            flow_id: flow,
            at: SystemTime::now(),
            findings: vec![email()],
        });
        tally.apply(&FlowEvent::Held {
            flow_id: flow,
            at: SystemTime::now(),
            deadline: std::time::Instant::now(),
            queue_bytes: 0,
            queue_count: 1,
        });
        tally.apply(&FlowEvent::Decided {
            flow_id: flow,
            at: SystemTime::now(),
            decision: Decision::Block {
                reason: BlockReason::User,
                note: Some("nicht ohne mich".to_owned()),
            },
            source: DecisionSource::User,
            findings: humanitl_core::DecidedFindings::default(),
        });
        tally.apply(&FlowEvent::Recorded {
            flow_id: flow,
            at: SystemTime::now(),
        });
        let counts = tally.finish();
        assert_eq!((counts.flows_total, counts.held, counts.blocked), (1, 1, 1));

        let (text, records) = records(writer);
        let kinds: Vec<&str> = records.iter().map(|r| r.body.kind.as_str()).collect();
        assert_eq!(kinds, ["flow.received", "flow.decided", "session.ended"]);
        assert_eq!(records[0].body.data["findings"], 1);
        assert_eq!(records[0].body.data["findings_kinds"][0], "email");
        assert_eq!(records[1].body.data["decision"], "block");
        for secret in [
            "alice@example.org",
            "quux-geheim",
            "/submit",
            "nicht ohne mich",
        ] {
            assert!(
                !text.contains(secret),
                "{secret} leaked into the log: {text}"
            );
        }
    }

    #[test]
    fn a_forwarded_flow_gets_a_response_record_and_a_rule_is_counted() {
        let (_dir, writer) = log();
        let mut tally = Tally::new(writer.handle(), SessionId::new());
        let flow = FlowId::new();
        let start = SystemTime::now();
        tally.apply(&received(flow, "/v1/models", b""));
        tally.apply(&FlowEvent::Analyzed {
            flow_id: flow,
            at: start,
            findings: Vec::new(),
        });
        tally.apply(&FlowEvent::Decided {
            flow_id: flow,
            at: start,
            decision: Decision::Allow,
            source: DecisionSource::Rule(RuleId::new()),
            findings: humanitl_core::DecidedFindings::default(),
        });
        tally.apply(&FlowEvent::Forwarded {
            flow_id: flow,
            at: start,
        });
        tally.apply(&FlowEvent::ResponseHeaders {
            flow_id: flow,
            at: start,
            status: 200,
        });
        for len in [10, 5] {
            tally.apply(&FlowEvent::ResponseChunk {
                flow_id: flow,
                at: start,
                len,
            });
        }
        tally.apply(&FlowEvent::Recorded {
            flow_id: flow,
            at: start + Duration::from_millis(1_250),
        });
        let counts = tally.finish();
        assert_eq!(counts.auto_rule, 1);

        let (_, records) = records(writer);
        let responded = records
            .iter()
            .find(|r| r.body.kind == "flow.responded")
            .expect("a response record");
        assert_eq!(responded.body.data["status"], 200);
        assert_eq!(responded.body.data["size"], 15);
        assert_eq!(responded.body.data["duration_ms"], 1_250);
        let decided = records
            .iter()
            .find(|r| r.body.kind == "flow.decided")
            .unwrap();
        assert_eq!(decided.body.data["decision"], "auto_allow");
    }

    #[test]
    fn a_timeout_is_one_decision_and_a_block_of_the_daemon_names_its_reason() {
        let (_dir, writer) = log();
        let mut tally = Tally::new(writer.handle(), SessionId::new());
        let slow = FlowId::new();
        tally.apply(&received(slow, "/", b""));
        tally.apply(&FlowEvent::TimedOut {
            flow_id: slow,
            at: SystemTime::now(),
        });
        // Ein zweites Wort über denselben Flow zählt nicht noch einmal.
        tally.apply(&FlowEvent::Decided {
            flow_id: slow,
            at: SystemTime::now(),
            decision: Decision::TimedOut,
            source: DecisionSource::Timeout,
            findings: humanitl_core::DecidedFindings::default(),
        });
        let big = FlowId::new();
        tally.apply(&received(big, "/upload", b""));
        tally.apply(&FlowEvent::Decided {
            flow_id: big,
            at: SystemTime::now(),
            decision: Decision::Block {
                reason: BlockReason::BodyCap,
                note: None,
            },
            source: DecisionSource::System,
            findings: humanitl_core::DecidedFindings::default(),
        });
        // Der gewöhnliche Weg einer Frist: nur `TimedOut`, kein `Decided`.
        let lone = FlowId::new();
        tally.apply(&received(lone, "/wait", b""));
        tally.apply(&FlowEvent::TimedOut {
            flow_id: lone,
            at: SystemTime::now(),
        });
        let counts = tally.finish();
        assert_eq!(
            (counts.timed_out, counts.blocked, counts.flows_total),
            (2, 1, 3)
        );

        let (_, records) = records(writer);
        let kinds: Vec<&str> = records.iter().map(|r| r.body.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "flow.received",
                "flow.decided",
                "flow.received",
                "flow.decided",
                "flow.blocked_reason",
                "flow.received",
                "flow.decided",
                "session.ended"
            ]
        );
        assert_eq!(records[1].body.data["decided_by"], "timeout");
        assert_eq!(records[4].body.data["reason"], "body_cap");
        assert_eq!(records[6].body.data["decision"], "timed_out");
        assert_eq!(records[7].body.data["timed_out"], 2);
    }

    /// `flow.decided` trägt beide Zahlen (HUM-160): wie viele Funde offen
    /// hinausgingen und wie viele davon der Mensch bestätigt hat. Ein Block
    /// trägt keine, denn nichts ging hinaus; und welcher Fund es war, steht
    /// nie im Log.
    #[test]
    fn flow_decided_carries_unresolved_and_acknowledged() {
        let (_dir, writer) = log();
        let mut tally = Tally::new(writer.handle(), SessionId::new());
        let decisions = [
            (
                Decision::Allow,
                humanitl_core::DecidedFindings {
                    unresolved: Some(0),
                    acknowledged: vec![0],
                },
            ),
            (
                Decision::Allow,
                humanitl_core::DecidedFindings::unresolved(1),
            ),
            (block_by_user(), humanitl_core::DecidedFindings::default()),
        ];
        for (decision, findings) in decisions {
            let flow = FlowId::new();
            tally.apply(&received(flow, "/", b"alice@example.org"));
            tally.apply(&FlowEvent::Decided {
                flow_id: flow,
                at: SystemTime::now(),
                decision,
                source: DecisionSource::User,
                findings,
            });
        }
        let _ended = tally.finish();

        let (_, records) = records(writer);
        let decided: Vec<(Option<u64>, Option<u64>)> = records
            .iter()
            .filter(|r| r.body.kind == "flow.decided")
            .map(|r| {
                (
                    r.body.data["unresolved_findings"].as_u64(),
                    r.body.data["acknowledged"].as_u64(),
                )
            })
            .collect();
        assert_eq!(
            decided,
            vec![(Some(0), Some(1)), (Some(1), Some(0)), (None, None)],
            "sent anyway, sent over the hold, blocked"
        );
    }

    /// Nimmt das System eine bearbeitete Freigabe zurück, steht das im Log:
    /// ein zweiter `flow.decided` als Block durch `system` ohne Zahl der Funde,
    /// der Grund, und die Sitzung zählt den Flow als Block (HUM-160).
    #[test]
    fn a_revised_allow_is_audited_as_a_block() {
        let (_dir, writer) = log();
        let mut tally = Tally::new(writer.handle(), SessionId::new());
        let flow = FlowId::new();
        tally.apply(&received(flow, "/", b"alice@example.org"));
        let edited = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
            "/",
        );
        tally.apply(&FlowEvent::Decided {
            flow_id: flow,
            at: SystemTime::now(),
            decision: Decision::AllowEdited {
                request: Box::new(edited),
            },
            source: DecisionSource::User,
            findings: humanitl_core::DecidedFindings::unresolved(1),
        });
        tally.apply(&FlowEvent::Decided {
            flow_id: flow,
            at: SystemTime::now(),
            decision: Decision::Block {
                reason: BlockReason::Secret,
                note: None,
            },
            source: DecisionSource::System,
            findings: humanitl_core::DecidedFindings::default(),
        });
        let counts = tally.finish();
        assert_eq!(
            (counts.allowed_edited, counts.blocked),
            (0, 1),
            "nothing went out, so the session counts a block"
        );

        let (_, records) = records(writer);
        let decided: Vec<&AuditRecord> = records
            .iter()
            .filter(|r| r.body.kind == "flow.decided")
            .collect();
        assert_eq!(decided.len(), 2, "the retraction has its own record");
        assert_eq!(decided[1].body.data["decision"], "block");
        assert_eq!(decided[1].body.data["decided_by"], "system");
        assert!(decided[1].body.data["unresolved_findings"].is_null());
        let reason = records
            .iter()
            .find(|r| r.body.kind == "flow.blocked_reason")
            .expect("the reason of the retraction");
        assert_eq!(reason.body.data["reason"], "secret");
    }

    fn block_by_user() -> Decision {
        Decision::Block {
            reason: BlockReason::User,
            note: None,
        }
    }
}
