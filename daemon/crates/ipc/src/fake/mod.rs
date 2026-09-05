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
use crate::convert::{after, before, diagnostic_to_proto, lagged_event, matches_filter, timestamp};
use crate::server_stub::{BoxStream, DaemonApi};
use crate::v1;
use crate::validate;
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
    /// Dieselbe Grenze, die der echte Dienst aus `limits.hold_body_cap_bytes`
    /// bekommt. Der Fake liest keine Konfiguration und nimmt deshalb die
    /// Vorgabe; ohne sie fehlte ihm die Prüfung ganz, und eine bearbeitete
    /// Anfrage beliebiger Größe käme durch, die der Daemon mit `IPC_004`
    /// ablehnt.
    body_cap_bytes: u64,
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
            body_cap_bytes: humanitl_config::Limits::default().hold_body_cap_bytes,
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

/// Die erfundenen Zeilen der Tabelle „Changed files".
///
/// Eigene Funktion, damit `fake_summary` lesbar bleibt: Die Zeilen sind Daten,
/// nicht Verhalten.
fn fake_changes() -> Vec<v1::FileChange> {
    vec![
        v1::FileChange {
            path: "fake/notes.md".to_owned(),
            path_hash: "00000000000000f1".to_owned(),
            mangled: false,
            kind: v1::FileChangeKind::Added as i32,
            size: 118,
            git_metadata: false,
            unprotected_by: String::new(),
            unscanned: v1::ScanSkip::Unspecified as i32,
        },
        v1::FileChange {
            path: ".git/hooks/pre-commit".to_owned(),
            path_hash: "00000000000000f2".to_owned(),
            mangled: false,
            kind: v1::FileChangeKind::Added as i32,
            size: 42,
            git_metadata: false,
            unprotected_by: ".git/hooks".to_owned(),
            unscanned: v1::ScanSkip::Unspecified as i32,
        },
        v1::FileChange {
            path: "fake/dump.bin".to_owned(),
            path_hash: "00000000000000f5".to_owned(),
            mangled: false,
            kind: v1::FileChangeKind::Added as i32,
            size: 9_000_000,
            git_metadata: false,
            unprotected_by: String::new(),
            // Die Zeile, die zeigt, was „nicht durchsucht" heisst: In
            // dieser Datei wurde nichts gefunden, weil in ihr nichts
            // gesucht wurde. Die Oberflaeche muss das anders zeichnen
            // als eine Datei ohne Fund.
            unscanned: v1::ScanSkip::TooLarge as i32,
        },
        v1::FileChange {
            path: ".git/index".to_owned(),
            path_hash: "00000000000000f3".to_owned(),
            mangled: false,
            kind: v1::FileChangeKind::Modified as i32,
            size: 4096,
            git_metadata: true,
            unprotected_by: String::new(),
            unscanned: v1::ScanSkip::Unspecified as i32,
        },
    ]
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
        // Ein Schlüssel, den niemand sortieren kann, wird abgelehnt und nicht
        // durch einen anderen ersetzt (`crate::validate::order_by`); vorher sah
        // der Fake nur nach, ob irgendwo `desc` stand.
        let (_, descending) = validate::order_by(&request.order_by)?;
        Ok(self.page(&request, descending))
    }

    async fn get_flow(&self, id: FlowId) -> Result<v1::FlowDetail, Diagnostic> {
        self.state
            .detail(id)
            .ok_or_else(|| not_held(id, "is unknown to the fake daemon"))
    }

    fn get_body(&self, body: v1::BodyRef) -> Result<BoxStream<v1::BodyChunk>, Diagnostic> {
        // Ein gekürzter Hash wurde vorher mit Nullen aufgefüllt und konnte
        // damit einen fremden Body treffen. Der echte Dienst lehnt ihn mit
        // `IPC_005` ab, und hier gilt dasselbe.
        let sha256 = validate::body_hash(&body)?;
        let data = self.state.blob(&sha256).unwrap_or_default();
        Ok(Box::pin(tokio_stream::iter(chunks(&data))))
    }

    async fn decide(&self, request: v1::DecideRequest) -> Result<v1::DecideResponse, Diagnostic> {
        // Dieselben Prüfungen in derselben Reihenfolge wie im echten Dienst,
        // und vor jeder Wirkung: eine Anfrage, die nicht ausführbar ist, legt
        // auch keine Regel an (`crate::validate`, CONVENTIONS 4.12).
        let decision = match validate::decide_plan(&request, self.body_cap_bytes)? {
            validate::DecidePlan::Decide(decision) => decision,
            validate::DecidePlan::RefuseEach(diagnostic) => {
                return Ok(v1::DecideResponse {
                    results: request
                        .flow_ids
                        .iter()
                        .map(|text| refused(text, &diagnostic))
                        .collect(),
                    created_rule_id: String::new(),
                    created_rule: None,
                });
            }
        };
        let created = self.remember(request.remember.as_ref())?;
        let mut results = Vec::with_capacity(request.flow_ids.len());
        let mut refusals = Vec::new();
        for text in &request.flow_ids {
            let (result, refusal) = self.decide_one(text, &decision);
            results.push(result);
            refusals.push(refusal);
        }
        if results.iter().all(|result| !result.applied) {
            // Nichts entschieden heißt: die Anfrage hat nichts bewirkt. Die
            // Regel, die nur zu dieser Entscheidung gehörte, wird deshalb
            // wieder zurückgenommen, und der Aufruf endet mit dem Befund des
            // ersten Flows — genau wie im echten Dienst.
            if let Some(rule) = created.as_ref() {
                self.forget(&rule.rule_id);
            }
            return Err(refusals.into_iter().flatten().next().unwrap_or_else(|| {
                Diagnostic::builder(codes::IPC_003, Severity::Error)
                    .why("no flow of this request could be decided".to_owned())
                    .build()
            }));
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
        // Ohne Operation ist es keine Anfrage, auch nicht im Fake: der echte
        // Dienst antwortet `IPC_005`, nicht mit der Liste.
        validate::rules_op(&request)?;
        self.apply_rules_op(request)
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

    /// Alle Zeilen des Doctors, keine davon gemessen (HUM-075).
    ///
    /// Der Fake hat keine Maschine, die er lesen könnte. Bis zum 2026-09-05
    /// meldete er dafür fünf grüne Zeilen — genau die Lüge, gegen die der
    /// echte Doctor gebaut ist: `ok`, weil niemand nachgesehen hat. Jetzt
    /// trägt jede Zeile `WARN` und den Befund `DOCTOR_012`, mit demselben Text
    /// wie überall im Fake. Eine Oberfläche, die gegen ihn übt, sieht damit
    /// den Zustand, den sie auch auf einem Rechner ohne Messung sehen würde.
    ///
    /// Die Kennungen sind [`humanitl_sandbox::doctor::CheckId::ALL`] und
    /// nicht eine eigene Liste: Sonst zeigte der Fake Zeilen, die es beim
    /// Daemon nicht gibt, oder ließe welche aus.
    async fn doctor(&self) -> v1::DoctorReport {
        use humanitl_sandbox::doctor::{CheckId, CheckOutcome};

        v1::DoctorReport {
            checks: CheckId::ALL
                .into_iter()
                .map(|id| {
                    crate::convert::doctor_check_to_proto(&CheckOutcome::unmeasured(
                        id,
                        NOTHING_MEASURED,
                        &["humanitl", "doctor"],
                    ))
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

    /// Antwortet, ohne den Endpunkt zu fragen — der Fake fasst kein Netz an.
    ///
    /// Die Modellnamen tragen deshalb den Vermerk `NOTHING_MEASURED`, und
    /// die Latenz bleibt 0: Eine Zahl sähe in einem Screenshot wie eine
    /// Messung aus, und der Fake misst nichts. Was er liefert, ist die Form
    /// der Antwort, damit die Oberfläche sie zeichnen kann.
    async fn probe_llm(
        &self,
        request: v1::ProbeLlmRequest,
    ) -> Result<v1::ProbeLlmResponse, Diagnostic> {
        // Was der echte Dienst nicht lesen kann, beantwortet auch der Fake
        // nicht mit einer erfundenen Modellliste: `LLM_007`, wie dort. Ein
        // leerer Endpunkt ist keine URL und wird nicht durch den der Sitzung
        // ersetzt — sonst übte die Oberfläche gegen einen Fehlerfall, den sie
        // nie zu sehen bekommt.
        let endpoint = validate::llm_endpoint(&request.endpoint)?.to_string();
        // Ob der Endpunkt privat ist, wird am Namen entschieden und nicht
        // geraten: Ein `api.openai.com` als privat auszuweisen wäre eine
        // Behauptung über die Welt, und der Fake misst nichts
        // (`backlog/CONVENTIONS.md` 4.13). Ohne Auflösung bleibt der Name als
        // Beleg, und alles andere zählt als nicht privat — die vorsichtige
        // Seite, denn nur dann warnt die Oberfläche mit `LLM_006`.
        let endpoint_is_private = looks_private(&endpoint);
        let mut diagnostics = Vec::new();
        if !endpoint_is_private && !endpoint.is_empty() {
            diagnostics.push(diagnostic_to_proto(
                &Diagnostic::builder(codes::LLM_006, Severity::Info)
                    .why(format!(
                        "{endpoint} is not on a private network. Traffic to this address \
                         bypasses the queue, so only put a machine you control here."
                    ))
                    .build(),
            ));
        }
        Ok(v1::ProbeLlmResponse {
            models: ["qwen2.5-coder:14b", "llama3.1:8b"]
                .into_iter()
                .map(|model| format!("{NOTHING_MEASURED}: {model}"))
                .collect(),
            flavor: v1::LlmProduct::Ollama as i32,
            diagnostic: diagnostics.first().cloned(),
            latency_ms: 0,
            diagnostics,
            endpoint_is_private,
        })
    }

    /// Die Zusammenfassung des simulierten Laufs.
    ///
    /// Dieselbe Reihenfolge wie beim echten Dienst: erst die Kennung lesen
    /// (`IPC_005`), dann nachsehen, ob es zu ihr etwas gibt (`SANDBOX_027`).
    /// Der Fake fuehrt genau einen Lauf; jede andere Kennung ist eine, zu der
    /// er nichts hat, und eine erfundene Antwort darauf brachte der Oberflaeche
    /// bei, dass es immer eine gibt.
    async fn get_session_summary(
        &self,
        request: v1::SessionSummaryRef,
    ) -> Result<v1::SessionSummary, Diagnostic> {
        let wanted = validate::sandbox_id(&request.sandbox_id)?;
        let (sandbox_id, _) = self.state.sandbox();
        if wanted != sandbox_id {
            return Err(crate::server::unknown_summary(wanted));
        }
        Ok(self.fake_summary())
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
    fn page(&self, request: &v1::ListFlowsRequest, descending: bool) -> v1::FlowPage {
        let since = FlowId::parse(&request.since_flow_id).ok();
        let cursor = FlowId::parse(&request.cursor).ok();
        // Der Cursor zeigt auf das letzte gelieferte Element; die nächste Seite liegt
        // in Sortierrichtung dahinter, also bei absteigender Reihenfolge davor.
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
    ///
    /// Die Regel läuft durch [`convert::rule_from_proto`] und wieder zurück,
    /// wie im echten Dienst. Das ist keine Formsache: Der Konverter erzwingt
    /// `bundled = false` und `passthrough_llm = false`. Der Fake legte die
    /// Nachricht vorher unverändert ab, und ein Client konnte sich damit eine
    /// mitgelieferte, unlöschbare Regel und eine Durchreichregel zum
    /// Sprachmodell bauen — beides verbietet `rules.proto`.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `IPC_005`, wenn die Regel nicht lesbar ist.
    fn remember(&self, rule: Option<&v1::Rule>) -> Result<Option<v1::Rule>, Diagnostic> {
        let Some(wire) = rule else {
            return Ok(None);
        };
        let mut wire = wire.clone();
        if wire.rule_id.is_empty() {
            wire.rule_id = RuleId::new().to_string();
        }
        let stored = convert::rule_to_proto(&self.read_rule(&wire)?);
        self.state.with_rules(|rules| rules.push(stored.clone()));
        self.state.emit_rules_changed(SystemTime::now());
        Ok(Some(stored))
    }

    /// Nimmt eine gerade angelegte Regel wieder zurück.
    fn forget(&self, rule_id: &str) {
        self.state
            .with_rules(|rules| rules.retain(|rule| rule.rule_id != rule_id));
        self.state.emit_rules_changed(SystemTime::now());
    }

    /// Entscheidet genau einen Flow.
    ///
    /// Der Befund kommt zusätzlich zum Ergebnis zurück, weil `Decide` ihn
    /// braucht, falls die Anfrage als Ganzes nichts bewirkt hat.
    fn decide_one(
        &self,
        text: &str,
        decision: &Decision,
    ) -> (v1::DecideResult, Option<Diagnostic>) {
        let id = match validate::flow_id(text) {
            Ok(id) => id,
            Err(diagnostic) => return (refused(text, &diagnostic), Some(diagnostic)),
        };
        if !self.state.is_held(id) {
            let reason = if self.state.knows(id) {
                "is no longer held"
            } else {
                "is unknown to the fake daemon"
            };
            let diagnostic = not_held(id, reason);
            return (refused(text, &diagnostic), Some(diagnostic));
        }

        let at = SystemTime::now();
        // Der Body einer bearbeiteten Anfrage wird abgelegt, damit `GetBody`
        // ihn liefert; alles andere hat [`crate::validate`] schon gelesen.
        if let Decision::AllowEdited { request } = decision {
            self.state.set_edited(id, (**request).clone());
        }
        let allow = decision.is_allow();
        if self
            .state
            .advance(
                id,
                humanitl_core::TransitionInput::Decide {
                    decision: decision.clone(),
                    source: DecisionSource::User,
                },
                at,
            )
            .is_err()
        {
            let diagnostic = not_held(id, "cannot be decided in its current state");
            return (refused(text, &diagnostic), Some(diagnostic));
        }
        if allow {
            self.state.complete_allowed(id, at);
        } else {
            self.state.complete_refused(id, at);
        }
        (
            v1::DecideResult {
                flow_id: text.to_owned(),
                applied: true,
                diagnostic: None,
            },
            None,
        )
    }

    /// Führt eine Regel-Operation aus.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`], wenn die Operation eine Regel oder eine Probe trägt, die
    /// sich nicht lesen lässt — mit denselben Codes wie im echten Dienst.
    fn apply_rules_op(&self, request: v1::RulesRequest) -> Result<v1::RulesResponse, Diagnostic> {
        let mut dry_run_matches = Vec::new();
        let mut test = None;
        match request.op {
            None | Some(v1::rules_request::Op::List(())) => {}
            // `add` und `update` gehen durch denselben Leser wie `remember`.
            // Ohne ihn legte der Fake die Nachricht der Leitung wörtlich ab,
            // und ein Client konnte sich hier eine mitgelieferte (unlöschbare)
            // Regel oder die Durchreiche zum Sprachmodell selbst geben —
            // dieselbe Lücke wie in `remember`, nur eine Tür weiter.
            Some(v1::rules_request::Op::Add(mut wire)) => {
                if wire.rule_id.is_empty() {
                    wire.rule_id = RuleId::new().to_string();
                }
                let rule = convert::rule_to_proto(&self.read_rule(&wire)?);
                self.state.with_rules(|rules| rules.push(rule));
                self.state.emit_rules_changed(SystemTime::now());
            }
            Some(v1::rules_request::Op::Update(wire)) => {
                let rule = convert::rule_to_proto(&self.read_rule(&wire)?);
                self.state.with_rules(|rules| {
                    if let Some(slot) = rules.iter_mut().find(|old| old.rule_id == rule.rule_id) {
                        *slot = rule;
                    }
                });
                self.state.emit_rules_changed(SystemTime::now());
            }
            // Eine mitgelieferte Regel ist unlöschbar. Der Fake behielt sie
            // stillschweigend und antwortete `Ok`; der echte Dienst sagt
            // `RULES_010` und schlägt eine eigene Regel davor vor. Der Befund
            // kommt aus derselben Funktion wie dort, damit es ihn nur einmal
            // gibt.
            Some(v1::rules_request::Op::Remove(id)) => {
                if let Some(bundled) = self
                    .state
                    .rules()
                    .iter()
                    .find(|rule| rule.rule_id == id && rule.bundled)
                {
                    return Err(humanitl_proxy::rules_store::immutable_bundled(
                        &self.read_rule(bundled)?,
                        "removed",
                    ));
                }
                self.state
                    .with_rules(|rules| rules.retain(|rule| rule.rule_id != id));
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
                // Ein Probelauf ohne lesbare Regel ist keiner. Der Fake lieferte
                // still eine leere Trefferliste, und die sieht aus wie „diese
                // Regel trifft nichts".
                let rule = self.read_rule(validate::dry_run_rule(&dry_run)?)?;
                dry_run_matches = self.dry_run(&convert::rule_to_proto(&rule), dry_run.limit);
            }
            Some(v1::rules_request::Op::Test(probe)) => {
                test = Some(self.test(&probe)?);
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
        Ok(v1::RulesResponse {
            rules: self.state.rules(),
            dry_run_scanned: u32::try_from(self.state.summaries().len()).unwrap_or(u32::MAX),
            dry_run_matches,
            diagnostic: None,
            diagnostics: Vec::new(),
            test,
        })
    }

    /// Liest eine Regel von der Leitung, mit denselben Codes wie der echte
    /// Dienst (`RULES_003` für ein Host-Muster, sonst `IPC_005`).
    fn read_rule(&self, wire: &v1::Rule) -> Result<humanitl_core::rule::Rule, Diagnostic> {
        validate::rule(wire, self.state.session().id)
    }

    /// Fragt den Regelsatz des Fakes, was er zu einer Anfrage sagt.
    ///
    /// Ausgewertet wird mit derselben Engine wie im Daemon: Die Oberfläche soll
    /// gegen den Fake nichts üben, was der echte Daemon anders beantwortet
    /// (`backlog/CONVENTIONS.md` 4.11). Eine Regel im Satz, die sich nicht
    /// lesen lässt, wird übergangen; ohne Treffer gilt `ask` — nie `allow`.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `IPC_005`, wenn Methode oder URL der Probe nicht
    /// lesbar sind. Vorher kam `Ok` mit leerem Ergebnis heraus, und das sieht
    /// für den Menschen aus wie „keine Regel trifft".
    fn test(&self, probe: &v1::rules_request::Test) -> Result<v1::RuleTest, Diagnostic> {
        let session = self.state.session().id;
        let (method, scheme, authority, path) = validate::rule_probe(probe)?;
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
        Ok(v1::RuleTest {
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
    fn dry_run(&self, rule: &v1::Rule, limit: u32) -> Vec<v1::FlowSummary> {
        let Some(pattern) = rule.matcher.as_ref() else {
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
                // Dieselbe Reihenfolge wie beim echten Dienst: Die drei
                // Garantien stehen zwischen `starting` und `running`, damit
                // niemand `running` sieht, bevor er die Ergebnisse gesehen
                // hat (HUM-041).
                let mut events = vec![self.sandbox_status(v1::SandboxState::Starting)];
                events.extend(isolation_checks());
                events.push(self.sandbox_status(v1::SandboxState::Running));
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
                    // Wie beim echten Dienst kommt die Zusammenfassung, sobald
                    // der Lauf endet, ohne dass ein Client danach fragt
                    // (HUM-043). Der Bildschirm haette sonst nichts zu zeigen.
                    v1::SandboxEvent {
                        event: Some(v1::sandbox_event::Event::Summary(self.fake_summary())),
                    },
                ]
            }
            Some(Op::IsolationCheck(())) => isolation_checks(),
            Some(Op::Argv(())) => argv_lines(&self.state.session().work_dir),
            Some(Op::Plan(plan)) => vec![self.sandbox_status_for(
                self.state.sandbox().1,
                Some(&plan.work_dir),
                Some(&plan.work_mode),
            )],
            _ => vec![self.sandbox_status(self.state.sandbox().1)],
        }
    }

    /// Die erfundene Zusammenfassung des simulierten Laufs (HUM-043).
    ///
    /// Vollstaendig, weil das Sheet sonst nichts zu zeigen haette, und
    /// unuebersehbar erfunden (CONVENTIONS 4.7): Die Pfade sagen `fake`. Sie
    /// deckt die drei Faelle ab, die die Oberflaeche unterscheiden muss —
    /// eine geaenderte Datei, ein Fund, ein Symlink nach draussen — und den
    /// Fall, dass ueber einem Pfad keine Maske lag.
    /// Gebaut wird die Wire-Form von Hand und nicht ueber
    /// `humanitl_sandbox::summary::SessionSummary`: Erfundene Daten gehoeren in
    /// den Fake, nicht in die Crate, die die echten baut.
    fn fake_summary(&self) -> v1::SessionSummary {
        let session = self.state.session();
        let (sandbox_id, _) = self.state.sandbox();
        v1::SessionSummary {
            session_id: session.id.to_string(),
            sandbox_id: sandbox_id.to_string(),
            created: Some(timestamp(SystemTime::now())),
            work_dir: session.work_dir.clone(),
            changes: fake_changes(),
            findings: vec![v1::SummaryFinding {
                path: "fake/notes.md".to_owned(),
                path_hash: "00000000000000f1".to_owned(),
                mangled: false,
                line: 3,
                kind: "api_key:fake".to_owned(),
                tier: v1::FindingTier::Regex as i32,
                display_prefix: "fake_key…".to_owned(),
                value_hash: "0".repeat(64),
            }],
            symlinks: vec![v1::SymlinkEscape {
                path: "fake/outside".to_owned(),
                path_hash: "00000000000000f4".to_owned(),
                mangled: false,
                target: "/etc".to_owned(),
                escapes: true,
                fix_command: format!("rm -- {}/fake/outside", session.work_dir),
            }],
            unprotected: vec![".git/hooks".to_owned(), ".idea".to_owned()],
            scanned_bytes: 160,
            truncated: false,
            diagnostics: vec![
                diagnostic_to_proto(
                    &Diagnostic::builder(codes::SANDBOX_022, Severity::Warning)
                        .why(
                            "the agent created a symlink fake/outside pointing outside the \
                             project (/etc); do not follow it"
                                .to_owned(),
                        )
                        .build(),
                ),
                diagnostic_to_proto(
                    &Diagnostic::builder(codes::SANDBOX_023, Severity::Warning)
                        .why(
                            "1 potential secret(s) were written into the project during this \
                             session"
                                .to_owned(),
                        )
                        .build(),
                ),
                diagnostic_to_proto(
                    &Diagnostic::builder(codes::SANDBOX_028, Severity::Warning)
                        .why(
                            "1 changed file(s) were not searched for secrets; the first is \
                             fake/dump.bin (larger than the scan reads). Nothing was found in \
                             them because nothing was looked at."
                                .to_owned(),
                        )
                        .build(),
                ),
            ],
        }
    }

    /// Der Zustand der simulierten Sandbox als Ereignis.
    ///
    /// Die Momentaufnahme ist vollstaendig, weil der Bildschirm sonst nichts
    /// zu zeigen haette: dieselben Einhaengungen, dieselbe Umgebung und
    /// dieselbe Kommandozeile, die ein echter Start haette, nur als Fake
    /// gekennzeichnet (CONVENTIONS 4.7).
    fn sandbox_status(&self, state: v1::SandboxState) -> v1::SandboxEvent {
        self.sandbox_status_for(state, None, None)
    }

    /// Dieselbe Momentaufnahme fuer ein Projektverzeichnis, das noch nicht
    /// gilt (`SandboxRequest.Plan`).
    fn sandbox_status_for(
        &self,
        state: v1::SandboxState,
        work_dir: Option<&str>,
        work_mode: Option<&str>,
    ) -> v1::SandboxEvent {
        let session = self.state.session();
        let (sandbox_id, _) = self.state.sandbox();
        let work_dir = work_dir
            .map(str::trim)
            .filter(|dir| !dir.is_empty())
            .map_or(session.work_dir, ToOwned::to_owned);
        let work_mode = work_mode
            .map(str::trim)
            .filter(|mode| matches!(*mode, "ro" | "rw"))
            .unwrap_or("rw")
            .to_owned();
        let running = matches!(
            state,
            v1::SandboxState::Running | v1::SandboxState::Stopping
        );
        v1::SandboxEvent {
            event: Some(v1::sandbox_event::Event::Status(
                v1::sandbox_event::Status {
                    state: state as i32,
                    sandbox_id: sandbox_id.to_string(),
                    session_id: session.id.to_string(),
                    backend: "bwrap".to_owned(),
                    llm_endpoint: session.llm_endpoint,
                    started_at: running.then(|| timestamp(SystemTime::now())),
                    profile: FAKE_PROFILE.to_owned(),
                    mounts: fake_mounts(&work_dir, &work_mode),
                    env: fake_env(),
                    argv_preview: fake_argv_line(&work_dir, &work_mode),
                    agent_running: running,
                    work_dir,
                    work_mode,
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
///
/// Ein Befund bleibt trotzdem, auch am durchgereichten Flow. `LLM_005` warnt
/// vor genau der Anfrage, die der Filter versteckt; sie mitzuverstecken
/// nähme dem Menschen die einzige Meldung, die er dazu bekommt (HUM-039). Der
/// echte Daemon macht es genauso (`IpcServer::event_stream`).
fn keeps(state: &FakeState, event: &v1::FlowEvent, include_passthrough: bool) -> bool {
    if include_passthrough || is_diagnostic(event) {
        return true;
    }
    event_flow_id(event).is_none_or(|id| !state.is_passthrough(id))
}

/// Ob eine Adresse nach dem eigenen Netz aussieht, allein nach dem Namen.
///
/// Dieselben Namensräume wie in `humanitl_proxy::llm_probe`, nur ohne die
/// Auflösung: Der Fake fasst kein Netz an, also kann er die Adresse hinter
/// einem Namen nicht kennen. Ein IP-Literal aus RFC 1918, Loopback,
/// Link-Local oder CGNAT zählt trotzdem, weil es sich selbst beantwortet.
fn looks_private(endpoint: &str) -> bool {
    let Ok(url) = url::Url::parse(endpoint) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    if let Ok(ip) = host.parse::<std::net::IpAddr>() {
        return humanitl_core::ip_is_private(ip);
    }
    let host = host.to_ascii_lowercase();
    host == "localhost"
        || [".local", ".lan", ".home.arpa", ".internal"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
}

/// Ob ein Wire-Ereignis ein Befund ist, mit oder ohne Flow.
fn is_diagnostic(event: &v1::FlowEvent) -> bool {
    matches!(
        event.event.as_ref(),
        Some(v1::flow_event::Event::Diagnostic(_) | v1::flow_event::Event::FlowDiagnostic(_))
    )
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
        Event::FlowDiagnostic(diagnostic) => diagnostic.flow_id.as_str(),
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

/// Das Sandbox-Profil, das der Fake vorgibt zu lesen.
const FAKE_PROFILE: &str = "default";

/// Die Einhaengungen, die ein echter Start haette. Dieselbe Liste, aus der
/// [`fake_argv_line`] ihre Zeile baut: eine Tabelle, die etwas anderes sagt
/// als die Kommandozeile daneben, waere schlimmer als keine.
fn fake_mounts(work_dir: &str, work_mode: &str) -> Vec<v1::Mount> {
    let work = if work_mode == "ro" {
        v1::MountMode::Ro
    } else {
        v1::MountMode::Rw
    };
    vec![
        mount("/usr", "/usr", v1::MountMode::Ro, v1::ValueOrigin::Profile),
        mount(
            "/etc/ssl",
            "/etc/ssl",
            v1::MountMode::Ro,
            v1::ValueOrigin::Profile,
        ),
        mount("", "/tmp", v1::MountMode::Tmpfs, v1::ValueOrigin::Profile),
        mount(
            "",
            "/dev/shm",
            v1::MountMode::Tmpfs,
            v1::ValueOrigin::Profile,
        ),
        mount("", "/proc", v1::MountMode::Proc, v1::ValueOrigin::Profile),
        mount("", "/dev", v1::MountMode::Dev, v1::ValueOrigin::Profile),
        mount(work_dir, "/work", work, v1::ValueOrigin::Session),
        mount(
            "",
            "/work/.git/config",
            v1::MountMode::Masked,
            v1::ValueOrigin::Profile,
        ),
        mount(
            "",
            "/work/.envrc",
            v1::MountMode::Masked,
            v1::ValueOrigin::Profile,
        ),
        mount(
            "$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock",
            "/run/humanitl/proxy.sock",
            v1::MountMode::Ro,
            v1::ValueOrigin::Session,
        ),
        mount(
            "$XDG_DATA_HOME/humanitl/ca/ca.crt",
            "/etc/humanitl/ca.crt",
            v1::MountMode::Ro,
            v1::ValueOrigin::Session,
        ),
        mount(
            "/usr/lib/humanitl/humanitl-shim",
            "/run/humanitl/humanitl-shim",
            v1::MountMode::Ro,
            v1::ValueOrigin::Session,
        ),
        mount(
            "",
            "/etc/humanitl/AGENTS.md",
            v1::MountMode::Masked,
            v1::ValueOrigin::Adapter,
        ),
    ]
}

/// Ein Eintrag der Einhaengetabelle.
fn mount(src: &str, dst: &str, mode: v1::MountMode, origin: v1::ValueOrigin) -> v1::Mount {
    v1::Mount {
        dst: dst.to_owned(),
        src: src.to_owned(),
        mode: mode as i32,
        origin: origin as i32,
        link_target: String::new(),
    }
}

/// Die Umgebung, die die Sandbox setzt, alphabetisch.
///
/// Zwei Werte sind zurueckgehalten, und beide sind mit Absicht so benannt, dass
/// keine Regel ueber verdaechtige Endungen sie faende: `AWS_ACCESS_KEY_ID`
/// endet auf `_ID`, `DATABASE_URL` traegt das Passwort in der URL. So zeigt
/// auch der Fake, dass die Vorgabe „zurueckgehalten" lautet (CONVENTIONS 4.17).
fn fake_env() -> Vec<v1::EnvVar> {
    [
        ("AWS_ACCESS_KEY_ID", "", v1::ValueOrigin::User, true),
        ("DATABASE_URL", "", v1::ValueOrigin::User, true),
        ("HOME", "/home/agent", v1::ValueOrigin::Profile, false),
        (
            "HTTPS_PROXY",
            "http://127.0.0.1:3128",
            v1::ValueOrigin::Profile,
            false,
        ),
        (
            "HTTP_PROXY",
            "http://127.0.0.1:3128",
            v1::ValueOrigin::Profile,
            false,
        ),
        ("NO_PROXY", "", v1::ValueOrigin::Profile, false),
        (
            "OPENCODE_CONFIG",
            "/etc/humanitl/opencode.json",
            v1::ValueOrigin::Adapter,
            false,
        ),
        ("PATH", "/usr/bin:/bin", v1::ValueOrigin::Profile, false),
        (
            "SSL_CERT_FILE",
            "/etc/humanitl/ca.crt",
            v1::ValueOrigin::Profile,
            false,
        ),
        ("TERM", "xterm-256color", v1::ValueOrigin::Profile, false),
        ("USER", "agent", v1::ValueOrigin::Profile, false),
    ]
    .into_iter()
    .map(|(key, value, origin, withheld)| v1::EnvVar {
        key: key.to_owned(),
        value: value.to_owned(),
        origin: origin as i32,
        withheld,
    })
    .collect()
}

/// Die Kommandozeile als eine Zeile, so wie eine Shell sie liest.
fn fake_argv_line(work_dir: &str, work_mode: &str) -> String {
    let work_flag = if work_mode == "ro" {
        "--ro-bind"
    } else {
        "--bind"
    };
    format!(
        "bwrap --unshare-all --die-with-parent --new-session --cap-drop ALL --disable-userns \
         --hostname sandbox --ro-bind /usr /usr --ro-bind /etc/ssl /etc/ssl --proc /proc --dev /dev \
         --tmpfs /tmp --tmpfs /dev/shm {work_flag} {work_dir} /work \
         --ro-bind $XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock /run/humanitl/proxy.sock \
         --ro-bind $XDG_DATA_HOME/humanitl/ca/ca.crt /etc/humanitl/ca.crt \
         --ro-bind /usr/lib/humanitl/humanitl-shim /run/humanitl/humanitl-shim \
         --clearenv --setenv HTTP_PROXY http://127.0.0.1:3128 \
         --setenv AWS_ACCESS_KEY_ID '<withheld>' \
         --setenv DATABASE_URL '<withheld>' --chdir /work \
         -- /run/humanitl/humanitl-shim --proxy-port 3128 -- opencode"
    )
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
