//! Der Fake-Daemon: dieselbe Schnittstelle, eine Datei statt eines Proxys.
//!
//! Der Fake erfüllt [`DaemonApi`] und läuft im echten Binary hinter dem echten
//! Socket (`humanitld --fake <session.jsonl>`). Die Oberfläche sieht keinen
//! Unterschied: Flows kommen mit ihrem Timing an, Holds sind echt (der
//! Zeitgeber läuft, `Decide` beendet den Hold, ein Ablauf blockt), Regeln
//! liegen im Speicher, Sandbox und Terminal antworten plausibel.
//!
//! Was der Fake nicht hat: einen Proxy, eine Sandbox, ein Sprachmodell, eine
//! Datenbank. Er ist das Werkzeug, mit dem die Oberfläche gebaut wird, bevor
//! es diese Dinge gibt (HUM-005).

pub mod player;
pub mod state;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use humanitl_config::{Sources, load};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Decision, DecisionSource, Diagnostic, FlowId, RuleId, Severity};
use tokio::sync::mpsc;
use tokio_stream::StreamExt as _;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::{BroadcastStream, UnboundedReceiverStream};

use crate::convert;
use crate::convert::{
    after, before, diagnostic_to_proto, lagged_event, matches_filter, request_from_proto, timestamp,
};
use crate::server_stub::{BoxStream, DaemonApi};
use crate::v1;
use crate::{PROTO_MAJOR, PROTO_MINOR};

pub use crate::convert::EditedRequestError;
pub use player::{PlayerOptions, Session, SessionError};
pub use state::{FakeFlow, FakeState, SessionMeta, StoredResponse};

use state::BODY_CHUNK_BYTES;

/// Die mitgelieferte Regel, die `models.dev` blockt.
///
/// Die Sitzungsdatei `fixtures/sessions/mixed.jsonl` verweist auf diese Id;
/// so zeigt die Oberfläche zu der automatischen Entscheidung die Regel, die
/// sie getroffen hat.
pub const BUNDLED_BLOCK_RULE: &str = "018f0000-0000-7000-8000-0000000000a1";

/// Die mitgelieferte Regel für die Durchreiche zum Sprachmodell.
pub const BUNDLED_PASSTHROUGH_RULE: &str = "018f0000-0000-7000-8000-0000000000a2";

/// Wie der Fake läuft.
#[derive(Debug, Clone, Copy)]
pub struct FakeOptions {
    /// Zeitraffer für die Zeitstempel der Datei.
    pub speed: f64,
    /// Die Datei nach dem Ende neu starten.
    pub repeat: bool,
    /// Auch die Wartezeiten raffen.
    pub scale_timeouts: bool,
    /// Die Wartezeit für `hold`-Zeilen ohne eigenen Wert.
    pub hold_timeout: Duration,
    /// Kapazität des Ereignis-Rundfunks (`limits.event_buffer`).
    pub event_buffer: usize,
}

impl Default for FakeOptions {
    fn default() -> Self {
        Self {
            speed: 1.0,
            repeat: false,
            scale_timeouts: false,
            hold_timeout: Duration::from_secs(300),
            event_buffer: 1024,
        }
    }
}

impl FakeOptions {
    /// Die Einstellungen des Abspielers, die daraus folgen.
    #[must_use]
    fn player(self) -> PlayerOptions {
        PlayerOptions {
            speed: self.speed,
            repeat: self.repeat,
            scale_timeouts: self.scale_timeouts,
            hold_timeout: self.hold_timeout,
        }
    }
}

/// Ein Daemon, der eine aufgezeichnete Sitzung spielt.
#[derive(Debug)]
pub struct FakeDaemon {
    state: Arc<FakeState>,
    session: Arc<Session>,
    options: FakeOptions,
}

impl FakeDaemon {
    /// Baut den Fake über einer eingelesenen Sitzung.
    #[must_use]
    pub fn new(session: Session, options: FakeOptions) -> Self {
        let meta = session.meta().unwrap_or_default();
        let state = Arc::new(FakeState::new(meta, options.event_buffer));
        state.set_rules(bundled_rules());
        Self {
            state,
            session: Arc::new(session),
            options,
        }
    }

    /// Der Zustand hinter dem Fake; gedacht für Tests.
    #[must_use]
    pub fn state(&self) -> &Arc<FakeState> {
        &self.state
    }

    /// Die eingelesene Sitzung.
    #[must_use]
    pub fn session(&self) -> &Arc<Session> {
        &self.session
    }

    /// Startet den Abspieler im Hintergrund.
    ///
    /// Der Abspieler läuft losgelöst weiter, auch wenn der Aufrufer das
    /// zurückgegebene Handle fallen lässt; es dient nur dem, der auf das
    /// Ende warten oder abbrechen will.
    // Das Handle ist absichtlich optional (Start-und-vergessen), ein
    // `#[must_use]` würde jeden Aufrufer zu einem `let _` zwingen.
    #[allow(clippy::must_use_candidate)]
    pub fn start(&self) -> tokio::task::JoinHandle<()> {
        let session = Arc::clone(&self.session);
        let state = Arc::clone(&self.state);
        let options = self.options.player();
        tokio::spawn(async move { player::play(session, state, options).await })
    }
}

/// Die Regeln, die der Fake von Anfang an kennt.
fn bundled_rules() -> Vec<v1::Rule> {
    vec![
        v1::Rule {
            rule_id: BUNDLED_BLOCK_RULE.to_owned(),
            action: v1::RuleAction::Block as i32,
            matcher: Some(v1::RuleMatcher {
                host: "models.dev".to_owned(),
                ..v1::RuleMatcher::default()
            }),
            expires: Some(never()),
            bundled: true,
            note: "Modellkatalog wird lokal mitgeliefert".to_owned(),
            ..v1::Rule::default()
        },
        v1::Rule {
            rule_id: BUNDLED_PASSTHROUGH_RULE.to_owned(),
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host: "ip:192.168.1.50".to_owned(),
                ..v1::RuleMatcher::default()
            }),
            expires: Some(never()),
            bundled: true,
            allow_private: true,
            note: "Durchreiche zum Sprachmodell".to_owned(),
            ..v1::Rule::default()
        },
    ]
}

/// Die Gültigkeit „für immer" in ihrer Wire-Form.
fn never() -> v1::RuleExpiry {
    v1::RuleExpiry {
        expiry: Some(v1::rule_expiry::Expiry::Never(())),
    }
}

/// Ein Befund, wenn ein Flow nicht mehr wartet oder gar nicht existiert.
fn not_held(id: FlowId, reason: &str) -> Diagnostic {
    Diagnostic::builder(codes::IPC_003, Severity::Error)
        .why(format!("flow {id} {reason}"))
        .build()
}

#[tonic::async_trait]
impl DaemonApi for FakeDaemon {
    async fn info(&self) -> v1::Info {
        v1::Info {
            daemon_version: env!("CARGO_PKG_VERSION").to_owned(),
            proto_major: PROTO_MAJOR,
            proto_minor: PROTO_MINOR,
            capabilities: vec![
                "fake".to_owned(),
                "proxy.h1".to_owned(),
                "findings.regex".to_owned(),
                "sandbox.bwrap".to_owned(),
            ],
            session_id: self.state.session().id.to_string(),
        }
    }

    fn subscribe(&self, request: v1::SubscribeRequest) -> BoxStream<v1::FlowEvent> {
        let include_passthrough = request.include_passthrough;
        let state = Arc::clone(&self.state);
        let live =
            BroadcastStream::new(self.state.subscribe()).filter_map(move |item| match item {
                Ok(event) if keeps(&state, &event, include_passthrough) => Some(event),
                Ok(_) => None,
                Err(BroadcastStreamRecvError::Lagged(dropped)) => Some(lagged_event(dropped)),
            });
        let backlog = self.backlog(&request);
        Box::pin(tokio_stream::iter(backlog).chain(live))
    }

    async fn list_flows(&self, request: v1::ListFlowsRequest) -> Result<v1::FlowPage, Diagnostic> {
        Ok(self.page(&request))
    }

    async fn get_flow(&self, id: FlowId) -> Result<v1::FlowDetail, Diagnostic> {
        self.state
            .detail(id)
            .ok_or_else(|| not_held(id, "is unknown to the fake daemon"))
    }

    fn get_body(&self, body: v1::BodyRef) -> BoxStream<v1::BodyChunk> {
        let mut sha256 = [0u8; 32];
        if body.sha256.len() == 32 {
            sha256.copy_from_slice(&body.sha256);
        }
        let data = self.state.blob(&sha256).unwrap_or_default();
        Box::pin(tokio_stream::iter(chunks(&data)))
    }

    async fn decide(&self, request: v1::DecideRequest) -> Result<v1::DecideResponse, Diagnostic> {
        let created = self.remember(request.remember.as_ref());
        let mut results = Vec::with_capacity(request.flow_ids.len());
        for text in &request.flow_ids {
            results.push(self.decide_one(text, &request));
        }
        Ok(v1::DecideResponse {
            results,
            created_rule_id: created
                .as_ref()
                .map(|rule| rule.rule_id.clone())
                .unwrap_or_default(),
            created_rule: created,
        })
    }

    async fn rules(&self, request: v1::RulesRequest) -> Result<v1::RulesResponse, Diagnostic> {
        Ok(self.apply_rules_op(request))
    }

    fn sandbox(&self, request: v1::SandboxRequest) -> BoxStream<v1::SandboxEvent> {
        Box::pin(tokio_stream::iter(self.sandbox_events(&request)))
    }

    fn terminal(&self, input: BoxStream<v1::TerminalInput>) -> BoxStream<v1::TerminalOutput> {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(echo_terminal(input, tx));
        Box::pin(UnboundedReceiverStream::new(rx))
    }

    async fn audit(&self, request: v1::AuditRequest) -> Result<v1::AuditResponse, Diagnostic> {
        let out_path = match request.op {
            Some(v1::audit_request::Op::Export(export)) => export.out_path,
            _ => String::new(),
        };
        Ok(v1::AuditResponse {
            ok: true,
            entries: 0,
            head_hash: Vec::new(),
            first_bad_seq: 0,
            out_path,
            diagnostic: None,
        })
    }

    async fn get_config(
        &self,
        request: v1::GetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic> {
        Self::config_snapshot(self.state.config_overrides(), request.include_schema)
    }

    async fn set_config(
        &self,
        request: v1::SetConfigRequest,
    ) -> Result<v1::ConfigSnapshot, Diagnostic> {
        let mut overrides = self.state.config_overrides();
        overrides.insert(request.key.clone(), request.value.clone());
        let snapshot = Self::config_snapshot(overrides, false)?;
        self.state.set_config(request.key, request.value);
        Ok(snapshot)
    }

    async fn doctor(&self) -> v1::DoctorReport {
        v1::DoctorReport {
            checks: ["bwrap", "userns", "seccomp", "runtime_dir", "llm"]
                .into_iter()
                .map(|id| v1::DoctorCheck {
                    id: id.to_owned(),
                    status: v1::CheckStatus::Ok as i32,
                    evidence: NOTHING_MEASURED.to_owned(),
                    diagnostic: None,
                })
                .collect(),
        }
    }

    /// Meldet den Endpunkt der Sitzungsdatei, sonst nichts Gemessenes.
    ///
    /// Host und Port kommen aus der Kopfzeile der Datei. Die Modelle hat
    /// niemand abgefragt, darum tragen sie den Vermerk aus
    /// `NOTHING_MEASURED`; die Latenz ist 0, denn eine Zahl sähe in einem
    /// Screenshot wie eine Messung aus.
    fn discover_llm(&self, _request: v1::DiscoverRequest) -> BoxStream<v1::DiscoverResult> {
        let endpoint = self.state.session().llm_endpoint;
        let (host, port) = split_endpoint(&endpoint);
        Box::pin(tokio_stream::iter(vec![v1::DiscoverResult {
            host,
            port,
            product: v1::LlmProduct::Ollama as i32,
            models: ["qwen2.5-coder:14b", "llama3.1:8b"]
                .into_iter()
                .map(|model| format!("{NOTHING_MEASURED}: {model}"))
                .collect(),
            latency_ms: 0,
            auth_required: false,
        }]))
    }
}

impl FakeDaemon {
    /// Die Ereignisse, die ein Abonnent vor dem Strom nachgeliefert bekommt.
    ///
    /// `since_flow_id` heißt: „ich kenne alles bis hierher". Der Fake liefert
    /// darum für jeden jüngeren Flow ein `Received`; den Rest holt sich der
    /// Client über `ListFlows`.
    fn backlog(&self, request: &v1::SubscribeRequest) -> Vec<v1::FlowEvent> {
        if request.since_flow_id.is_empty() {
            return Vec::new();
        }
        let Ok(since) = FlowId::parse(&request.since_flow_id) else {
            return Vec::new();
        };
        self.state
            .summaries()
            .into_iter()
            .filter(|summary| {
                FlowId::parse(&summary.flow_id).is_ok_and(|id| id > since)
                    && (request.include_passthrough || !summary.passthrough)
            })
            .map(|summary| v1::FlowEvent {
                at: summary.received_at,
                event: Some(v1::flow_event::Event::Received(v1::flow_event::Received {
                    summary: Some(summary),
                    domain: None,
                })),
            })
            .collect()
    }

    /// Eine Seite der Flow-Historie.
    fn page(&self, request: &v1::ListFlowsRequest) -> v1::FlowPage {
        let since = FlowId::parse(&request.since_flow_id).ok();
        let cursor = FlowId::parse(&request.cursor).ok();
        // Der Cursor zeigt auf das letzte gelieferte Element; die nächste Seite liegt
        // in Sortierrichtung dahinter, also bei absteigender Reihenfolge davor.
        let descending = request.order_by.contains("desc");
        let mut flows: Vec<v1::FlowSummary> = self
            .state
            .summaries()
            .into_iter()
            .filter(|summary| request.include_passthrough || !summary.passthrough)
            .filter(|summary| matches_filter(summary, &request.filter))
            .filter(|summary| after(summary, since))
            .filter(|summary| {
                if descending {
                    before(summary, cursor)
                } else {
                    after(summary, cursor)
                }
            })
            .collect();
        if descending {
            flows.reverse();
        }

        let total = u64::try_from(flows.len()).unwrap_or(u64::MAX);
        let limit = match request.limit {
            0 => 200,
            other => usize::try_from(other.min(1000)).unwrap_or(200),
        };
        let next_cursor = if flows.len() > limit {
            flows
                .get(limit - 1)
                .map(|summary| summary.flow_id.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        flows.truncate(limit);
        v1::FlowPage {
            flows,
            next_cursor,
            total,
            // Der Fake haelt alles im Speicher und zaehlt genau.
            capped: false,
        }
    }

    /// Legt die Regel aus `remember` an, falls eine mitkam.
    fn remember(&self, rule: Option<&v1::Rule>) -> Option<v1::Rule> {
        let mut rule = rule?.clone();
        if rule.rule_id.is_empty() {
            rule.rule_id = RuleId::new().to_string();
        }
        self.state.with_rules(|rules| rules.push(rule.clone()));
        self.state.emit_rules_changed(SystemTime::now());
        Some(rule)
    }

    /// Entscheidet genau einen Flow.
    fn decide_one(&self, text: &str, request: &v1::DecideRequest) -> v1::DecideResult {
        let Ok(id) = FlowId::parse(text) else {
            return refused(
                text,
                &Diagnostic::builder(codes::IPC_003, Severity::Error)
                    .why(format!("{text} is not a flow id"))
                    .build(),
            );
        };
        let edited = matches!(
            request.decision,
            Some(v1::decide_request::Decision::AllowEdited(_))
        );
        if edited && request.flow_ids.len() > 1 {
            return refused(
                text,
                &Diagnostic::builder(codes::IPC_002, Severity::Error)
                    .why(format!(
                        "allow_edited came with {} flow ids",
                        request.flow_ids.len()
                    ))
                    .build(),
            );
        }
        if !self.state.is_held(id) {
            let reason = if self.state.knows(id) {
                "is no longer held"
            } else {
                "is unknown to the fake daemon"
            };
            return refused(text, &not_held(id, reason));
        }

        let at = SystemTime::now();
        let decision = match &request.decision {
            Some(v1::decide_request::Decision::Block(block)) => {
                // Die Notiz erreicht den Agenten im 403-Body und im Header; sie wird
                // deshalb wie im echten Daemon gesäubert (HUM-072, CONVENTIONS 4.11).
                let note = humanitl_core::block::sanitize_note(&block.note);
                Decision::Block {
                    reason: humanitl_core::BlockReason::User,
                    note: (!note.is_empty()).then_some(note),
                }
            }
            Some(v1::decide_request::Decision::AllowEdited(edited)) => {
                // Eine bearbeitete Anfrage, die sich nicht lesen lässt, wird
                // nicht stillschweigend zur unbearbeiteten: das hieße, etwas
                // durchzulassen, was der Mensch so nie gesehen hat. Der Body
                // reist in der `EditedRequest` vollständig mit und wird mit
                // der Anfrage abgelegt, damit `GetBody` ihn liefert.
                let request = match request_from_proto(edited) {
                    Ok(request) => request,
                    Err(error) => {
                        return refused(
                            text,
                            &Diagnostic::builder(codes::IPC_004, Severity::Error)
                                .why(format!("the edited request is not readable: {error}"))
                                .build(),
                        );
                    }
                };
                self.state.set_edited(id, request.clone());
                Decision::AllowEdited {
                    request: Box::new(request),
                }
            }
            Some(v1::decide_request::Decision::Allow(())) => Decision::Allow,
            // Keine Entscheidung ist keine Freigabe. Der Fake steht in Tests an
            // der Stelle des Daemons; liesse er eine leere Anfrage als `Allow`
            // durch, uebte die Oberflaeche gegen ein Verhalten, das der echte
            // Daemon mit `IPC_004` ablehnt, und der Unterschied fiele erst im
            // Betrieb auf.
            None => {
                return refused(
                    text,
                    &Diagnostic::builder(codes::IPC_004, Severity::Error)
                        .why("the decide request carries no decision".to_owned())
                        .build(),
                );
            }
        };
        let allow = decision.is_allow();
        if self
            .state
            .advance(
                id,
                humanitl_core::TransitionInput::Decide {
                    decision,
                    source: DecisionSource::User,
                },
                at,
            )
            .is_err()
        {
            return refused(
                text,
                &not_held(id, "cannot be decided in its current state"),
            );
        }
        if allow {
            self.state.complete_allowed(id, at);
        } else {
            self.state.complete_refused(id, at);
        }
        v1::DecideResult {
            flow_id: text.to_owned(),
            applied: true,
            diagnostic: None,
        }
    }

    /// Führt eine Regel-Operation aus.
    fn apply_rules_op(&self, request: v1::RulesRequest) -> v1::RulesResponse {
        let mut dry_run_matches = Vec::new();
        let mut test = None;
        match request.op {
            None | Some(v1::rules_request::Op::List(())) => {}
            Some(v1::rules_request::Op::Add(mut rule)) => {
                if rule.rule_id.is_empty() {
                    rule.rule_id = RuleId::new().to_string();
                }
                self.state.with_rules(|rules| rules.push(rule));
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::Update(rule)) => {
                self.state.with_rules(|rules| {
                    if let Some(slot) = rules.iter_mut().find(|old| old.rule_id == rule.rule_id) {
                        *slot = rule;
                    }
                });
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::Remove(id)) => {
                self.state
                    .with_rules(|rules| rules.retain(|rule| rule.rule_id != id || rule.bundled));
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::Reorder(order)) => {
                self.state
                    .with_rules(|rules| reorder(rules, &order.rule_ids_in_order));
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::MakePermanent(id)) => {
                self.state.with_rules(|rules| {
                    if let Some(rule) = rules.iter_mut().find(|rule| rule.rule_id == id) {
                        rule.expires = Some(never());
                    }
                });
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::DryRun(dry_run)) => {
                dry_run_matches = self.dry_run(dry_run.rule.as_ref(), dry_run.limit);
            }
            Some(v1::rules_request::Op::Test(probe)) => {
                test = self.test(&probe);
            }
            // Der Fake hat keine Datei: neu zu laden heißt hier, den Stand zu
            // melden, den er ohnehin hat. Ein `UNIMPLEMENTED` wäre falsch,
            // weil die Oberfläche gegen den Fake übt, was der Daemon kann.
            Some(v1::rules_request::Op::Reload(())) => {
                self.state.emit_rules_changed(SystemTime::now());
            }
            // Der Fake meldet dieselben Codes wie der echte Daemon
            // (`backlog/CONVENTIONS.md` 4.12): abschalten geht nur bei
            // mitgelieferten Regeln, alles andere bleibt unverändert.
            Some(v1::rules_request::Op::SetDisabled(request)) => {
                self.state.with_rules(|rules| {
                    if let Some(rule) = rules
                        .iter_mut()
                        .find(|rule| rule.rule_id == request.rule_id && rule.bundled)
                    {
                        rule.disabled = request.disabled;
                    }
                });
                self.state.emit_rules_changed(SystemTime::now());
            }
        }
        v1::RulesResponse {
            rules: self.state.rules(),
            dry_run_scanned: u32::try_from(self.state.summaries().len()).unwrap_or(u32::MAX),
            dry_run_matches,
            diagnostic: None,
            diagnostics: Vec::new(),
            test,
        }
    }

    /// Fragt den Regelsatz des Fakes, was er zu einer Anfrage sagt.
    ///
    /// Ausgewertet wird mit derselben Engine wie im Daemon: Die Oberfläche soll
    /// gegen den Fake nichts üben, was der echte Daemon anders beantwortet
    /// (`backlog/CONVENTIONS.md` 4.11). Eine Regel, die sich nicht lesen lässt,
    /// wird übergangen; eine unlesbare Probe ergibt keinen Treffer, und ohne
    /// Treffer gilt `ask` — nie `allow`.
    fn test(&self, probe: &v1::rules_request::Test) -> Option<v1::RuleTest> {
        let session = self.state.session().id;
        let method = convert::method_from_proto(probe.method, "").ok()?;
        let (scheme, authority, path) = convert::split_url(&probe.url).ok()?;
        let rules = self.state.rules();
        let set = humanitl_rules::RuleSet::from_rules(
            rules
                .iter()
                .filter_map(|rule| convert::rule_from_proto(rule, session).ok()),
        );
        let mut key = humanitl_rules::RequestKey::new(
            &authority.host,
            &method,
            &path,
            scheme,
            authority.port,
        );
        if v1::Upgrade::try_from(probe.upgrade) == Ok(v1::Upgrade::Websocket) {
            key = key.with_upgrade(humanitl_core::Upgrade::WebSocket);
        }
        let verdict = set.evaluate(&key, chrono::Utc::now(), session);
        let matching = verdict
            .rule()
            .and_then(|id| rules.iter().find(|rule| rule.rule_id == id.to_string()));
        Some(v1::RuleTest {
            action: convert::action_to_proto(verdict.action()) as i32,
            matched: matches!(verdict, humanitl_rules::Verdict::Matched { .. }),
            rule_id: verdict.rule().map(|id| id.to_string()).unwrap_or_default(),
            position: matching.map_or(0, |rule| rule.position),
        })
    }

    /// Probelauf einer Regel gegen die bekannten Flows.
    ///
    /// Der Fake vergleicht nur den Host, und zwar wörtlich oder als Suffix
    /// eines `**`-Musters. Die richtige Auswertung steht in `humanitl-rules`;
    /// hier geht es darum, dass die Oberfläche eine Liste zum Anzeigen hat.
    fn dry_run(&self, rule: Option<&v1::Rule>, limit: u32) -> Vec<v1::FlowSummary> {
        let Some(pattern) = rule.and_then(|rule| rule.matcher.as_ref()) else {
            return Vec::new();
        };
        let limit = match limit {
            0 => 500,
            other => usize::try_from(other).unwrap_or(500),
        };
        self.state
            .summaries()
            .into_iter()
            .filter(|summary| {
                summary
                    .authority
                    .as_ref()
                    .is_some_and(|authority| host_matches(&pattern.host, &authority.host))
            })
            .take(limit)
            .collect()
    }

    /// Die Ereignisse, die eine Sandbox-Operation erzeugt.
    fn sandbox_events(&self, request: &v1::SandboxRequest) -> Vec<v1::SandboxEvent> {
        use v1::sandbox_request::Op;

        match &request.op {
            Some(Op::Start(start)) => {
                self.state.set_sandbox_state(v1::SandboxState::Running);
                let mut events = vec![self.sandbox_status(v1::SandboxState::Starting)];
                events.push(self.sandbox_status(v1::SandboxState::Running));
                events.extend(isolation_checks());
                events.push(sandbox_log(format!(
                    "started {} in {}",
                    start.command.join(" "),
                    start.work_dir
                )));
                events
            }
            Some(Op::Stop(())) => {
                self.state.set_sandbox_state(v1::SandboxState::Stopped);
                vec![
                    self.sandbox_status(v1::SandboxState::Stopping),
                    self.sandbox_status(v1::SandboxState::Stopped),
                ]
            }
            Some(Op::IsolationCheck(())) => isolation_checks(),
            Some(Op::Argv(())) => argv_lines(&self.state.session().work_dir),
            _ => vec![self.sandbox_status(self.state.sandbox().1)],
        }
    }

    /// Der Zustand der simulierten Sandbox als Ereignis.
    fn sandbox_status(&self, state: v1::SandboxState) -> v1::SandboxEvent {
        let session = self.state.session();
        let (sandbox_id, _) = self.state.sandbox();
        v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::Status(
                v1::sandbox_event::Status {
                    state: state as i32,
                    sandbox_id: sandbox_id.to_string(),
                    session_id: session.id.to_string(),
                    backend: "bwrap".to_owned(),
                    llm_endpoint: session.llm_endpoint,
                    work_dir: session.work_dir,
                    work_mode: "rw".to_owned(),
                },
            )),
        }
    }

    /// Die effektive Konfiguration des Fakes.
    ///
    /// Vorgabewerte aus `humanitl-config` plus die Schlüssel, die über
    /// `SetConfig` gesetzt wurden. Der Fake liest keine Datei und keine
    /// Umgebungsvariable: er soll auf jeder Maschine dasselbe zeigen.
    ///
    /// # Errors
    ///
    /// Der Befund aus `humanitl-config`, wenn ein gesetzter Schlüssel nicht im
    /// Schema steht (`CONFIG_002`) oder sein Wert nicht passt (`CONFIG_003`).
    fn config_snapshot(
        overrides: BTreeMap<String, String>,
        include_schema: bool,
    ) -> Result<v1::ConfigSnapshot, Diagnostic> {
        let resolved = load(&Sources::empty().with_cli(overrides))?;
        let toml = toml::to_string_pretty(&resolved.config).map_err(|error| {
            Diagnostic::builder(codes::CONFIG_001, Severity::Error)
                .why(format!(
                    "the effective configuration is not valid TOML: {error}"
                ))
                .build()
        })?;
        Ok(v1::ConfigSnapshot {
            toml,
            json_schema: if include_schema {
                humanitl_config::json_schema().to_string()
            } else {
                String::new()
            },
            origins: resolved
                .origins
                .iter()
                .map(|(key, origin)| v1::FieldOrigin {
                    key: key.clone(),
                    origin: origin.kind().to_owned(),
                })
                .collect(),
            diagnostics: resolved
                .diagnostics
                .iter()
                .map(diagnostic_to_proto)
                .collect(),
        })
    }
}

/// Ein abgelehntes Einzelergebnis einer Entscheidung.
fn refused(flow_id: &str, diagnostic: &Diagnostic) -> v1::DecideResult {
    v1::DecideResult {
        flow_id: flow_id.to_owned(),
        applied: false,
        diagnostic: Some(diagnostic_to_proto(diagnostic)),
    }
}

/// Ob ein Ereignis den Filter des Abonnenten übersteht.
///
/// Ohne `include_passthrough` fällt der gesamte Verkehr zum Sprachmodell weg,
/// nicht nur sein `Received`: sonst bekäme die Oberfläche Ereignisse zu einem
/// Flow, den sie nie gesehen hat.
fn keeps(state: &FakeState, event: &v1::FlowEvent, include_passthrough: bool) -> bool {
    if include_passthrough {
        return true;
    }
    event_flow_id(event).is_none_or(|id| !state.is_passthrough(id))
}

/// Der Flow, zu dem ein Wire-Ereignis gehört.
fn event_flow_id(event: &v1::FlowEvent) -> Option<FlowId> {
    use v1::flow_event::Event;

    let text = match event.event.as_ref()? {
        Event::Received(received) => received.summary.as_ref()?.flow_id.as_str(),
        Event::Analyzed(analyzed) => analyzed.flow_id.as_str(),
        Event::Held(held) => held.flow_id.as_str(),
        Event::Decided(decided) => decided.flow_id.as_str(),
        Event::ResponseHeaders(headers) => headers.flow_id.as_str(),
        Event::ResponseChunk(chunk) => chunk.flow_id.as_str(),
        Event::Failed(failed) => failed.flow_id.as_str(),
        Event::Forwarded(reference) | Event::Recorded(reference) | Event::TimedOut(reference) => {
            reference.flow_id.as_str()
        }
        Event::Lagged(_) | Event::Diagnostic(_) | Event::RulesChanged(_) | Event::AgentAsk(_) => {
            return None;
        }
    };
    FlowId::parse(text).ok()
}

/// Ob ein Host zu einem Muster passt, grob.
fn host_matches(pattern: &str, host: &str) -> bool {
    match pattern.strip_prefix("**.") {
        Some(apex) => host == apex || host.ends_with(&format!(".{apex}")),
        None => match pattern.strip_prefix("*.") {
            Some(apex) => host.ends_with(&format!(".{apex}")),
            None => pattern == host || pattern.strip_prefix("ip:") == Some(host),
        },
    }
}

/// Ordnet die Regeln nach einer Liste von Ids um.
fn reorder(rules: &mut Vec<v1::Rule>, order: &[String]) {
    let mut sorted = Vec::with_capacity(rules.len());
    for id in order {
        if let Some(position) = rules.iter().position(|rule| &rule.rule_id == id) {
            sorted.push(rules.remove(position));
        }
    }
    sorted.append(rules);
    *rules = sorted;
}

/// Der Body in Stücken von 64 KiB; ein leerer Body ist genau ein Stück.
fn chunks(data: &[u8]) -> Vec<v1::BodyChunk> {
    if data.is_empty() {
        return vec![v1::BodyChunk {
            data: Vec::new(),
            offset: 0,
            last: true,
        }];
    }
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        let end = (offset + BODY_CHUNK_BYTES).min(data.len());
        out.push(v1::BodyChunk {
            data: data[offset..end].to_vec(),
            offset: u64::try_from(offset).unwrap_or(u64::MAX),
            last: end == data.len(),
        });
        offset = end;
    }
    out
}

/// Das Präfix jedes Belegs aus dem Fake: hier wurde nichts gemessen.
///
/// Wie in [`DaemonApi::doctor`]; ein Beleg, der wie eine echte Messung
/// aussieht, wäre in einem Screenshot oder einem Fehlerbericht nicht mehr von
/// einer zu unterscheiden.
const NOTHING_MEASURED: &str = "fake daemon: nothing was measured";

/// Ein Beleg, der sagt, was der echte Daemon ausführen würde.
fn would_run(command: &str) -> String {
    format!("{NOTHING_MEASURED} (would run: {command})")
}

/// Die drei Garantien, im Fake immer grün und als Fake gekennzeichnet.
fn isolation_checks() -> Vec<v1::SandboxEvent> {
    [
        (v1::IsolationCheck::NoNetworkInterface, "ip link"),
        (v1::IsolationCheck::SingleSocket, "ss -x"),
        (
            v1::IsolationCheck::SeccompActive,
            "grep Seccomp /proc/<agent>/status",
        ),
    ]
    .into_iter()
    .map(|(check, command)| v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Check(v1::CheckResult {
            check: check as i32,
            passed: true,
            evidence: would_run(command),
            diagnostic: None,
        })),
    })
    .collect()
}

/// Eine Zeile im Log der simulierten Sandbox.
fn sandbox_log(line: String) -> v1::SandboxEvent {
    v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::Log(v1::sandbox_event::LogLine {
            at: Some(timestamp(SystemTime::now())),
            line,
        })),
    }
}

/// Die Kommandozeile, die der echte Launcher bauen würde, Zeile für Zeile als
/// Fake gekennzeichnet.
fn argv_lines(work_dir: &str) -> Vec<v1::SandboxEvent> {
    [
        "bwrap".to_owned(),
        "--unshare-all".to_owned(),
        "--cap-drop ALL".to_owned(),
        "--hostname sandbox".to_owned(),
        format!("--bind {work_dir} /work"),
        "--ro-bind /usr /usr".to_owned(),
        "--tmpfs /tmp".to_owned(),
        "--bind $XDG_RUNTIME_DIR/humanitl/proxy/<session>.sock /run/humanitl/proxy.sock".to_owned(),
        "/usr/local/bin/humanitl-shim".to_owned(),
    ]
    .into_iter()
    .map(|line| v1::SandboxEvent {
        event: Some(v1::sandbox_event::Event::ArgvLine(would_run(&line))),
    })
    .collect()
}

/// Spiegelt die Eingabe des Terminals zurück.
async fn echo_terminal(
    mut input: BoxStream<v1::TerminalInput>,
    out: mpsc::UnboundedSender<v1::TerminalOutput>,
) {
    use v1::terminal_input::Input;
    use v1::terminal_output::Output;

    while let Some(item) = input.next().await {
        let sent = match item.input {
            Some(Input::Open(open)) => {
                let resize = Output::Resize(v1::terminal_output::Resize {
                    cols: open.cols.max(80),
                    rows: open.rows.max(24),
                });
                out.send(v1::TerminalOutput {
                    output: Some(resize),
                })
                .and_then(|()| {
                    out.send(v1::TerminalOutput {
                        output: Some(Output::Data(
                            b"humanitl fake terminal: input is echoed\r\n".to_vec(),
                        )),
                    })
                })
            }
            Some(Input::Data(data)) => out.send(v1::TerminalOutput {
                output: Some(Output::Data(data)),
            }),
            Some(Input::Resize(resize)) => out.send(v1::TerminalOutput {
                output: Some(Output::Resize(v1::terminal_output::Resize {
                    cols: resize.cols,
                    rows: resize.rows,
                })),
            }),
            Some(Input::Close(())) | None => {
                let _ = out.send(v1::TerminalOutput {
                    output: Some(Output::Exit(v1::terminal_output::Exit { code: 0 })),
                });
                return;
            }
        };
        if sent.is_err() {
            return;
        }
    }
}

/// Trennt `http://host:port` in Host und Port.
fn split_endpoint(endpoint: &str) -> (String, u32) {
    let rest = endpoint
        .split_once("://")
        .map_or(endpoint, |(_, rest)| rest)
        .trim_end_matches('/');
    let rest = rest.split('/').next().unwrap_or(rest);
    match rest.rsplit_once(':') {
        Some((host, port)) => (host.to_owned(), port.parse().unwrap_or(11434)),
        None => (rest.to_owned(), 11434),
    }
}
