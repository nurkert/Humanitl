//! Die Sitzungsdatei und ihr Abspieler.
//!
//! Eine Sitzung ist eine JSONL-Datei: eine Zeile je Ereignis, `t_ms` relativ
//! zum Start. Der Abspieler wartet bis zum jeweiligen Zeitpunkt und treibt
//! damit denselben Zustandsautomaten, den der echte Proxy treiben würde. Was
//! die Datei nicht enthält, sind Entscheidungen über gehaltene Flows: die
//! kommen vom Client oder von der ablaufenden Wartezeit.
//!
//! Das Format steht in `fixtures/sessions/README.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use bytes::Bytes;
use humanitl_core::diagnostics::codes;
use humanitl_core::http::{
    Authority, BodyRef, HeaderMap, HeaderName, HeaderValue, HttpRequest, Method, Scheme,
};
use humanitl_core::{
    Decision, DecisionSource, Diagnostic, Finding, FindingKind, FindingLocation, FlowId, FlowState,
    HostName, RuleId, SessionId, Severity, Tier, TransitionInput,
};
use serde::Deserialize;
use uuid::Uuid;

use super::state::{FakeFlow, FakeState, SessionMeta, StoredResponse, deadline_instant};

/// Eine Sitzungsdatei ließ sich nicht lesen.
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// Die Datei war nicht lesbar.
    #[error("cannot read session file {path}: {source}")]
    Read {
        /// Der Pfad, der nicht gelesen werden konnte.
        path: PathBuf,
        /// Der Fehler des Dateisystems.
        source: std::io::Error,
    },
    /// Eine Zeile war kein gültiges JSON oder kein bekanntes Ereignis.
    #[error("session line {line}: {source}")]
    Json {
        /// Die Zeilennummer, ab 1.
        line: usize,
        /// Der Fehler des JSON-Lesers.
        source: serde_json::Error,
    },
    /// Eine Zeile war JSON, aber ihr Inhalt ergab keinen Sinn.
    #[error("session line {line}: {reason}")]
    Field {
        /// Die Zeilennummer, ab 1.
        line: usize,
        /// Was nicht stimmte.
        reason: String,
    },
}

impl SessionError {
    /// Ein Befund, den die Oberfläche und die Kommandozeile zeigen können.
    ///
    /// Der Code ist `CONFIG_001`: eine Sitzungsdatei ist für den Fake das,
    /// was `config.toml` für den Daemon ist — die Eingabe, ohne die er nicht
    /// startet. Ein eigener Bereich für ein Entwicklungswerkzeug wäre eine
    /// Nummer, die im fertigen Programm niemand je sieht.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::builder(codes::CONFIG_001, Severity::Blocking)
            .title("Sitzungsdatei ungültig")
            .why(self.to_string())
            .build()
    }
}

/// Eine Zeile der Sitzungsdatei.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionLine {
    /// Zeitpunkt relativ zum Start der Sitzung, in Millisekunden.
    #[serde(default)]
    pub t_ms: u64,
    /// Was zu diesem Zeitpunkt geschieht.
    #[serde(flatten)]
    pub kind: LineKind,
}

/// Die Ereignisarten einer Sitzungsdatei.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LineKind {
    /// Kopfzeile der Sitzung.
    Session(SessionSpec),
    /// Eine Anfrage trifft ein.
    Request(RequestSpec),
    /// Die Detektoren sind gelaufen.
    Findings(FindingsSpec),
    /// Die Anfrage wird gehalten.
    Hold(HoldSpec),
    /// Eine Regel oder die Durchreiche entscheidet ohne den Menschen.
    Auto(AutoSpec),
    /// Das Ziel hat geantwortet.
    Response(ResponseSpec),
    /// Ein vollständiger Durchreiche-Flow in einer Zeile.
    Passthrough(PassthroughSpec),
    /// Ein sitzungsweiter Befund.
    Diagnostic(DiagnosticSpec),
}

/// Kopfzeile der Sitzung.
#[derive(Debug, Clone, Deserialize)]
pub struct SessionSpec {
    /// Id der Sitzung, als UUID-Text.
    pub session_id: String,
    /// Der LLM-Endpunkt der Sitzung.
    #[serde(default)]
    pub llm_endpoint: String,
    /// Das Projektverzeichnis der Sitzung.
    #[serde(default)]
    pub work_dir: String,
}

/// Eine eintreffende Anfrage.
#[derive(Debug, Clone, Deserialize)]
pub struct RequestSpec {
    /// Id des Flows, als UUID-Text.
    pub flow_id: String,
    /// Die Methode, zum Beispiel `GET`.
    pub method: String,
    /// Das Schema; ohne Angabe `https`.
    #[serde(default)]
    pub scheme: Option<String>,
    /// Der Ziel-Host.
    pub host: String,
    /// Der Ziel-Port; ohne Angabe der Standard-Port des Schemas.
    #[serde(default)]
    pub port: Option<u16>,
    /// Pfad samt Query.
    pub path: String,
    /// Die Kopfzeilen, als Paare.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Der Body, Base64-kodiert.
    #[serde(default)]
    pub body_b64: Option<String>,
    /// Der Body als Text; bequemer für handgeschriebene Fixtures.
    #[serde(default)]
    pub body: Option<String>,
    /// Das Werkzeug, das die Anfrage gestellt hat.
    #[serde(default)]
    pub origin_tool: Option<String>,
    /// Ein Protokollwechsel, zum Beispiel `websocket`.
    #[serde(default)]
    pub upgrade: Option<String>,
}

/// Die Funde zu einer Anfrage.
#[derive(Debug, Clone, Deserialize)]
pub struct FindingsSpec {
    /// Id des Flows.
    pub flow_id: String,
    /// Die Funde.
    #[serde(default)]
    pub findings: Vec<FindingSpec>,
}

/// Ein einzelner Fund.
#[derive(Debug, Clone, Deserialize)]
pub struct FindingSpec {
    /// Art, zum Beispiel `api_key.github`, `email`, `jwt`.
    pub kind: String,
    /// Ort: `body`, `query` oder `header:<name>`.
    pub location: String,
    /// Sicherheit: `checksum`, `regex` oder `user_term`.
    #[serde(default)]
    pub tier: Option<String>,
    /// Der gefundene Wert. Er wird gehasht und gekürzt, nie gespeichert.
    pub value: String,
    /// Byte-Bereich innerhalb des Orts; ohne Angabe ab 0.
    #[serde(default)]
    pub span: Option<(usize, usize)>,
}

/// Ein Flow wird gehalten.
#[derive(Debug, Clone, Deserialize)]
pub struct HoldSpec {
    /// Id des Flows.
    pub flow_id: String,
    /// Wartezeit in Millisekunden; ohne Angabe der Wert aus der Konfiguration.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Eine Entscheidung ohne den Menschen.
#[derive(Debug, Clone, Deserialize)]
pub struct AutoSpec {
    /// Id des Flows.
    pub flow_id: String,
    /// Herkunft: `rule` oder `passthrough`.
    #[serde(default)]
    pub source: Option<String>,
    /// Id der Regel, die entschieden hat.
    #[serde(default)]
    pub rule_id: Option<String>,
    /// `allow` oder `block`.
    pub kind: String,
    /// Notiz für den Agenten, nur bei `block`.
    #[serde(default)]
    pub note: Option<String>,
}

/// Die Antwort des Ziels.
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseSpec {
    /// Id des Flows.
    pub flow_id: String,
    /// Der Status der Antwort.
    pub status: u16,
    /// Die Kopfzeilen der Antwort.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Der Body, Base64-kodiert.
    #[serde(default)]
    pub body_b64: Option<String>,
    /// Der Body als Text.
    #[serde(default)]
    pub body: Option<String>,
    /// Ob die Antwort gestreamt wird.
    #[serde(default)]
    pub streaming: bool,
}

/// Ein vollständiger Durchreiche-Flow.
#[derive(Debug, Clone, Deserialize)]
pub struct PassthroughSpec {
    /// Id des Flows.
    pub flow_id: String,
    /// Die Methode.
    pub method: String,
    /// Das Schema; ohne Angabe `http`, weil der LLM-Endpunkt meist lokal ist.
    #[serde(default)]
    pub scheme: Option<String>,
    /// Der Ziel-Host.
    pub host: String,
    /// Der Ziel-Port.
    #[serde(default)]
    pub port: Option<u16>,
    /// Pfad samt Query.
    pub path: String,
    /// Die Kopfzeilen der Anfrage.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    /// Der Anfrage-Body, Base64-kodiert.
    #[serde(default)]
    pub body_b64: Option<String>,
    /// Der Anfrage-Body als Text.
    #[serde(default)]
    pub body: Option<String>,
    /// Der Status der Antwort.
    #[serde(default)]
    pub response_status: Option<u16>,
    /// Der Antwort-Body als Text.
    #[serde(default)]
    pub response_body: Option<String>,
}

/// Ein sitzungsweiter Befund.
#[derive(Debug, Clone, Deserialize)]
pub struct DiagnosticSpec {
    /// Der Code aus dem Register, zum Beispiel `TLS_001`.
    pub code: String,
    /// Dringlichkeit: `info`, `warning`, `error` oder `blocking`.
    #[serde(default)]
    pub severity: Option<String>,
    /// Der veränderliche Teil der Meldung.
    pub why: String,
    /// Der Behebungsvorschlag.
    #[serde(default)]
    pub fix: Option<FixSpec>,
}

/// Ein Behebungsvorschlag in der Sitzungsdatei.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixSpec {
    /// Eine Umgebungsvariable setzen.
    SetEnv {
        /// Name der Variable.
        key: String,
        /// Wert der Variable.
        value: String,
    },
    /// Eine Einstellung ändern.
    ChangeSetting {
        /// Schlüssel im Schema.
        key: String,
        /// Der vorgeschlagene Wert.
        value: String,
    },
    /// Einen Befehl in die Zwischenablage legen.
    CopyCommand(String),
    /// Eine Adresse öffnen.
    OpenUrl(String),
}

/// Eine eingelesene Sitzung.
#[derive(Debug, Clone)]
pub struct Session {
    lines: Vec<SessionLine>,
}

impl Session {
    /// Liest eine Sitzung aus einer Datei.
    ///
    /// # Errors
    ///
    /// [`SessionError`], wenn die Datei fehlt oder eine Zeile nicht passt.
    pub fn load(path: &Path) -> Result<Self, SessionError> {
        let text = std::fs::read_to_string(path).map_err(|source| SessionError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text)
    }

    /// Liest eine Sitzung aus dem Inhalt einer Datei.
    ///
    /// Leere Zeilen und Zeilen, die mit `#` beginnen, sind Kommentare.
    ///
    /// # Errors
    ///
    /// [`SessionError`], wenn eine Zeile kein gültiges Ereignis ist.
    pub fn parse(text: &str) -> Result<Self, SessionError> {
        let mut lines = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let trimmed = raw.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let line: SessionLine =
                serde_json::from_str(trimmed).map_err(|source| SessionError::Json {
                    line: index + 1,
                    source,
                })?;
            check_line(index + 1, &line)?;
            lines.push(line);
        }
        lines.sort_by_key(|line| line.t_ms);
        Ok(Self { lines })
    }

    /// Die Zeilen in Abspielreihenfolge.
    #[must_use]
    pub fn lines(&self) -> &[SessionLine] {
        &self.lines
    }

    /// Wie lange die Sitzung dauert, in Millisekunden.
    #[must_use]
    pub fn span_ms(&self) -> u64 {
        self.lines.last().map_or(0, |line| line.t_ms)
    }

    /// Die Kopfzeile der Sitzung, falls sie eine hat.
    #[must_use]
    pub fn meta(&self) -> Option<SessionMeta> {
        self.lines.iter().find_map(|line| match &line.kind {
            LineKind::Session(spec) => Some(SessionMeta {
                id: SessionId::parse(&spec.session_id).unwrap_or_else(|_| SessionId::new()),
                llm_endpoint: spec.llm_endpoint.clone(),
                work_dir: spec.work_dir.clone(),
            }),
            _ => None,
        })
    }

    /// Alle Flow-Ids der Datei, in Textform, in Reihenfolge.
    #[must_use]
    pub fn flow_id_texts(&self) -> Vec<&str> {
        self.lines
            .iter()
            .filter_map(|line| match &line.kind {
                LineKind::Request(spec) => Some(spec.flow_id.as_str()),
                LineKind::Passthrough(spec) => Some(spec.flow_id.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Prüft die Felder einer Zeile, die serde nicht prüfen kann.
fn check_line(number: usize, line: &SessionLine) -> Result<(), SessionError> {
    let field = |reason: String| SessionError::Field {
        line: number,
        reason,
    };
    match &line.kind {
        LineKind::Session(spec) => SessionId::parse(&spec.session_id)
            .map(|_| ())
            .map_err(|err| field(err.to_string())),
        LineKind::Request(spec) => check_flow_id(&spec.flow_id)
            .and_then(|()| check_body_b64(spec.body_b64.as_deref()))
            .map_err(field),
        LineKind::Passthrough(spec) => check_flow_id(&spec.flow_id)
            .and_then(|()| check_body_b64(spec.body_b64.as_deref()))
            .map_err(field),
        LineKind::Findings(spec) => check_flow_id(&spec.flow_id).map_err(field),
        LineKind::Hold(spec) => check_flow_id(&spec.flow_id).map_err(field),
        LineKind::Auto(spec) => check_flow_id(&spec.flow_id).map_err(field),
        LineKind::Response(spec) => check_flow_id(&spec.flow_id)
            .and_then(|()| check_body_b64(spec.body_b64.as_deref()))
            .map_err(field),
        LineKind::Diagnostic(spec) => {
            if humanitl_core::diag::lookup_str(&spec.code).is_some() {
                Ok(())
            } else {
                Err(field(format!(
                    "{} is not in the diagnostic registry",
                    spec.code
                )))
            }
        }
    }
}

/// Prüft eine Flow-Id auf ihre Textform.
fn check_flow_id(text: &str) -> Result<(), String> {
    FlowId::parse(text)
        .map(|_| ())
        .map_err(|err| err.to_string())
}

/// Prüft, dass ein `body_b64` gültiges Base64 ist.
///
/// Ein Tippfehler in der Datei endet so beim Lesen mit Zeile und Feld, nicht
/// später als stiller leerer Body. Ein leeres `body_b64` gilt wie ein
/// fehlendes ([`body_of`]).
fn check_body_b64(encoded: Option<&str>) -> Result<(), String> {
    match encoded {
        Some(encoded) if !encoded.is_empty() => decode_body_b64(encoded)
            .map(|_| ())
            .map_err(|err| format!("body_b64 is not valid base64: {err}")),
        _ => Ok(()),
    }
}

/// Liest einen Base64-Body (Standard-Alphabet, mit Auffüllung).
fn decode_body_b64(encoded: &str) -> Result<Bytes, base64::DecodeError> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map(Bytes::from)
}

/// Wie eine Sitzung abgespielt wird.
#[derive(Debug, Clone, Copy)]
pub struct PlayerOptions {
    /// Zeitraffer: alle `t_ms` werden durch diesen Wert geteilt.
    pub speed: f64,
    /// Die Datei nach dem Ende neu starten, mit neuen Flow-Ids.
    pub repeat: bool,
    /// Auch die Wartezeiten mit `speed` raffen.
    pub scale_timeouts: bool,
    /// Die Wartezeit für `hold`-Zeilen ohne eigenen Wert.
    pub hold_timeout: Duration,
}

impl Default for PlayerOptions {
    fn default() -> Self {
        Self {
            speed: 1.0,
            repeat: false,
            scale_timeouts: false,
            hold_timeout: Duration::from_secs(300),
        }
    }
}

impl PlayerOptions {
    /// Rafft eine Dauer um [`PlayerOptions::speed`].
    ///
    /// Ein Wert, mit dem sich nicht rechnen lässt (`NaN`, unendlich, null,
    /// negativ) oder dessen Ergebnis keine `Duration` mehr ist, lässt die
    /// Dauer, wie sie ist: die Kommandozeile prüft `--speed` schon beim
    /// Lesen, ein Aufrufer der Bibliothek muss das nicht, und eine Panik in
    /// `Duration::div_f64` gibt es hier so oder so nicht.
    #[must_use]
    fn scaled(&self, span: Duration) -> Duration {
        if !self.speed.is_finite() || self.speed <= 0.0 {
            return span;
        }
        if (self.speed - 1.0).abs() < f64::EPSILON {
            return span;
        }
        Duration::try_from_secs_f64(span.as_secs_f64() / self.speed).unwrap_or(span)
    }

    /// Der Abstand einer Zeile vom Start, gerafft.
    #[must_use]
    fn offset(&self, t_ms: u64) -> Duration {
        self.scaled(Duration::from_millis(t_ms))
    }

    /// Die Wartezeit eines Holds, gerafft nur mit `scale_timeouts`.
    #[must_use]
    fn hold_span(&self, timeout_ms: Option<u64>) -> Duration {
        let span = timeout_ms.map_or(self.hold_timeout, Duration::from_millis);
        if self.scale_timeouts {
            self.scaled(span)
        } else {
            span
        }
    }
}

/// Was der Abspieler vor dem Start über die Datei weiß.
#[derive(Debug, Default)]
struct Index {
    with_findings: BTreeSet<String>,
    responses: BTreeMap<String, ResponseSpec>,
}

impl Index {
    /// Baut die Nachschlagetabellen einer Sitzung.
    fn build(session: &Session) -> Self {
        let mut index = Self::default();
        for line in &session.lines {
            match &line.kind {
                LineKind::Findings(spec) => {
                    index.with_findings.insert(spec.flow_id.clone());
                }
                LineKind::Response(spec) => {
                    index.responses.insert(spec.flow_id.clone(), spec.clone());
                }
                _ => {}
            }
        }
        index
    }
}

/// Ein Durchlauf der Datei.
///
/// Alle Ids eines Durchlaufs leiten sich von seinem Zeitstempel ab, nicht von
/// der Uhr beim Lesen der jeweiligen Zeile. Sonst bekäme eine `hold`- oder
/// `response`-Zeile im zweiten Durchlauf eine andere Id als ihre
/// `request`-Zeile, sobald zwischen beiden die Millisekunde wechselt, und der
/// Übergang ginge still verloren.
#[derive(Debug, Clone, Copy)]
struct Pass {
    /// Nummer des Durchlaufs, ab 0.
    iteration: u32,
    /// Zeitstempel des Durchlaufs: Millisekunden seit der Epoche, von
    /// Durchlauf zu Durchlauf streng steigend ([`PassStamps`]).
    stamp_ms: u64,
}

impl Pass {
    /// Die Wanduhrzeit, von der die Zeitpunkte des Durchlaufs ausgehen.
    fn started_at(self) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(self.stamp_ms)
    }
}

/// Vergibt jedem Durchlauf seinen Zeitstempel.
///
/// Der Stempel ist die Wanduhr in Millisekunden, mindestens aber eine
/// Millisekunde mehr als beim Durchlauf davor. Mit einem hohen `--speed` und
/// einer kurzen Datei liegen zwei Durchläufe sonst in derselben Millisekunde,
/// bekämen dieselben Ids, und der zweite fände die Flows des ersten vor.
#[derive(Debug, Default)]
struct PassStamps {
    last_ms: Option<u64>,
}

impl PassStamps {
    /// Der nächste Durchlauf, gestempelt mit `now` oder später.
    fn next(&mut self, iteration: u32, now: SystemTime) -> Pass {
        let now_ms = now.duration_since(UNIX_EPOCH).map_or(0u64, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        });
        let stamp_ms = match self.last_ms {
            Some(last) => now_ms.max(last.saturating_add(1)),
            None => now_ms,
        };
        self.last_ms = Some(stamp_ms);
        Pass {
            iteration,
            stamp_ms,
        }
    }
}

/// Spielt eine Sitzung ab, bis sie zu Ende ist oder für immer.
pub async fn play(session: Arc<Session>, state: Arc<FakeState>, options: PlayerOptions) {
    let index = Index::build(&session);
    let mut stamps = PassStamps::default();
    let mut iteration: u32 = 0;
    loop {
        let pass = stamps.next(iteration, SystemTime::now());
        play_once(&session, &state, &index, options, pass).await;
        if !options.repeat {
            return;
        }
        iteration = iteration.saturating_add(1);
        tokio::time::sleep(options.scaled(Duration::from_millis(500))).await;
    }
}

/// Spielt die Datei genau einmal ab.
async fn play_once(
    session: &Session,
    state: &Arc<FakeState>,
    index: &Index,
    options: PlayerOptions,
    pass: Pass,
) {
    let start = tokio::time::Instant::now();
    let started_at = pass.started_at();
    for line in &session.lines {
        tokio::time::sleep_until(start + options.offset(line.t_ms)).await;
        let at = started_at + options.offset(line.t_ms);
        apply_line(state, index, options, line, pass, at);
    }
}

/// Wendet eine Zeile auf den Zustand an.
fn apply_line(
    state: &Arc<FakeState>,
    index: &Index,
    options: PlayerOptions,
    line: &SessionLine,
    pass: Pass,
    at: SystemTime,
) {
    match &line.kind {
        LineKind::Session(spec) => {
            state.set_session(SessionMeta {
                id: SessionId::parse(&spec.session_id).unwrap_or_else(|_| SessionId::new()),
                llm_endpoint: spec.llm_endpoint.clone(),
                work_dir: spec.work_dir.clone(),
            });
        }
        LineKind::Request(spec) => play_request(state, index, spec, pass, at),
        LineKind::Findings(spec) => {
            let id = flow_id(&spec.flow_id, pass);
            let findings = spec.findings.iter().map(build_finding).collect();
            let _ = state.advance(id, TransitionInput::Analyze { findings }, at);
        }
        LineKind::Hold(spec) => play_hold(state, options, spec, pass, at),
        LineKind::Auto(spec) => play_auto(state, index, spec, pass, at),
        LineKind::Response(spec) => {
            let id = flow_id(&spec.flow_id, pass);
            if matches!(state.state_of(id), Some(FlowState::Forwarded)) {
                state.arm_response(id, build_response(spec));
                state.complete_forwarded(id, at);
            }
        }
        LineKind::Passthrough(spec) => play_passthrough(state, spec, pass, at),
        LineKind::Diagnostic(spec) => state.emit_diagnostic(&build_diagnostic(spec), at),
    }
}

/// Nimmt eine Anfrage auf und analysiert sie, wenn keine Fund-Zeile folgt.
fn play_request(
    state: &Arc<FakeState>,
    index: &Index,
    spec: &RequestSpec,
    pass: Pass,
    at: SystemTime,
) {
    let id = flow_id(&spec.flow_id, pass);
    let session = state.session().id;
    let mut flow = FakeFlow::new(id, session, at, build_request(spec));
    flow.origin_tool.clone_from(&spec.origin_tool);
    state.receive(flow);

    if let Some(response) = index.responses.get(&spec.flow_id) {
        state.arm_response(id, build_response(response));
    }
    if !index.with_findings.contains(&spec.flow_id) {
        let _ = state.advance(
            id,
            TransitionInput::Analyze {
                findings: Vec::new(),
            },
            at,
        );
    }
}

/// Hält eine Anfrage und stellt den Zeitgeber.
fn play_hold(
    state: &Arc<FakeState>,
    options: PlayerOptions,
    spec: &HoldSpec,
    pass: Pass,
    at: SystemTime,
) {
    let id = flow_id(&spec.flow_id, pass);
    ensure_analyzed(state, id, at);
    let span = options.hold_span(spec.timeout_ms);
    state.set_deadline(id, at + span);
    if state
        .advance(
            id,
            TransitionInput::Hold {
                deadline: deadline_instant(span),
                queue_bytes: 0,
                queue_count: 0,
            },
            at,
        )
        .is_err()
    {
        return;
    }
    let timer_state = Arc::clone(state);
    tokio::spawn(async move {
        tokio::time::sleep(span).await;
        timer_state.time_out(id, SystemTime::now());
    });
}

/// Entscheidet ohne den Menschen und spielt die Folgen.
fn play_auto(state: &Arc<FakeState>, index: &Index, spec: &AutoSpec, pass: Pass, at: SystemTime) {
    let id = flow_id(&spec.flow_id, pass);
    ensure_analyzed(state, id, at);

    let rule = spec
        .rule_id
        .as_deref()
        .and_then(|text| RuleId::parse(text).ok());
    let source = match spec.source.as_deref() {
        Some("passthrough") => DecisionSource::Passthrough,
        _ => DecisionSource::Rule(rule.unwrap_or_else(RuleId::nil)),
    };
    let allow = spec.kind != "block";
    let decision = if allow {
        Decision::Allow
    } else {
        Decision::Block {
            reason: rule.map_or(humanitl_core::BlockReason::User, |id| {
                humanitl_core::BlockReason::Rule(id)
            }),
            note: spec.note.clone(),
        }
    };

    if state
        .advance(id, TransitionInput::Decide { decision, source }, at)
        .is_err()
    {
        return;
    }
    if !allow {
        state.complete_refused(id, at);
        return;
    }
    if state.advance(id, TransitionInput::Forward, at).is_err() {
        return;
    }
    if !index.responses.contains_key(&spec.flow_id) {
        state.complete_forwarded(id, at);
    }
}

/// Spielt einen vollständigen Durchreiche-Flow in einem Zug.
fn play_passthrough(state: &Arc<FakeState>, spec: &PassthroughSpec, pass: Pass, at: SystemTime) {
    let id = flow_id(&spec.flow_id, pass);
    let session = state.session().id;
    let request = passthrough_request(spec);
    let mut flow = FakeFlow::new(id, session, at, request);
    flow.passthrough = true;
    state.receive(flow);

    let body = text_body(spec.response_body.as_deref(), None);
    state.arm_response(
        id,
        StoredResponse {
            status: spec.response_status.unwrap_or(200),
            headers: header_map(&[("content-type".to_owned(), "application/json".to_owned())]),
            body,
            streaming: false,
        },
    );

    let steps = [
        TransitionInput::Analyze {
            findings: Vec::new(),
        },
        TransitionInput::Decide {
            decision: Decision::Allow,
            source: DecisionSource::Passthrough,
        },
    ];
    for step in steps {
        if state.advance(id, step, at).is_err() {
            return;
        }
    }
    state.complete_allowed(id, at);
}

/// Analysiert einen Flow, der noch nicht analysiert wurde.
fn ensure_analyzed(state: &FakeState, id: FlowId, at: SystemTime) {
    if matches!(state.state_of(id), Some(FlowState::Received)) {
        let _ = state.advance(
            id,
            TransitionInput::Analyze {
                findings: Vec::new(),
            },
            at,
        );
    }
}

/// Die Flow-Id einer Zeile in diesem Durchlauf.
///
/// Im ersten Durchlauf ist es die Id aus der Datei. Danach behält die Id ihre
/// zufälligen Bytes — daran erkennt man den Flow der Vorlage wieder — und die
/// 48 Bit Zeit werden durch den Zeitstempel des Durchlaufs ersetzt
/// (CONVENTIONS.md 4.7). So bleibt die Ordnung der Ids die Ordnung der Zeit,
/// auf die sich `ListFlows(since)` verlässt, und jede Zeile eines Durchlaufs
/// findet ihren Flow wieder, weil alle vom selben Stempel ausgehen.
fn flow_id(text: &str, pass: Pass) -> FlowId {
    let base = FlowId::parse(text).unwrap_or_else(|_| FlowId::nil());
    if pass.iteration == 0 {
        return base;
    }
    let mut bytes = *base.as_uuid().as_bytes();
    let stamp = pass.stamp_ms.to_be_bytes();
    bytes[0..6].copy_from_slice(&stamp[2..8]);
    FlowId::from_uuid(Uuid::from_bytes(bytes))
}

/// Baut die Anfrage einer `request`-Zeile.
fn build_request(spec: &RequestSpec) -> HttpRequest {
    let scheme = scheme_of(spec.scheme.as_deref(), Scheme::Https);
    let mut headers = header_map(&spec.headers);
    if spec.upgrade.as_deref() == Some("websocket") {
        add_missing(&mut headers, "connection", "Upgrade");
        add_missing(&mut headers, "upgrade", "websocket");
    }
    let body = body_of(spec.body_b64.as_deref(), spec.body.as_deref(), &headers);
    HttpRequest::new(
        method_of(&spec.method),
        scheme,
        authority_of(&spec.host, spec.port, scheme),
        spec.path.clone(),
    )
    .with_headers(headers)
    .with_body(body)
}

/// Baut die Anfrage einer `passthrough`-Zeile.
fn passthrough_request(spec: &PassthroughSpec) -> HttpRequest {
    let scheme = scheme_of(spec.scheme.as_deref(), Scheme::Http);
    let headers = header_map(&spec.headers);
    let body = body_of(spec.body_b64.as_deref(), spec.body.as_deref(), &headers);
    HttpRequest::new(
        method_of(&spec.method),
        scheme,
        authority_of(&spec.host, spec.port, scheme),
        spec.path.clone(),
    )
    .with_headers(headers)
    .with_body(body)
}

/// Baut die Antwort einer `response`-Zeile.
fn build_response(spec: &ResponseSpec) -> StoredResponse {
    let headers = header_map(&spec.headers);
    StoredResponse {
        status: spec.status,
        body: body_of(spec.body_b64.as_deref(), spec.body.as_deref(), &headers),
        headers,
        streaming: spec.streaming,
    }
}

/// Baut einen Befund einer `diagnostic`-Zeile.
fn build_diagnostic(spec: &DiagnosticSpec) -> Diagnostic {
    let code =
        humanitl_core::diag::lookup_str(&spec.code).map_or(codes::CONFIG_001, |info| info.code);
    let severity = match spec.severity.as_deref() {
        Some("info") => Severity::Info,
        Some("error") => Severity::Error,
        Some("blocking") => Severity::Blocking,
        _ => Severity::Warning,
    };
    let builder = Diagnostic::builder(code, severity).why(spec.why.clone());
    match &spec.fix {
        None => builder.build(),
        Some(fix) => builder.fix(build_fix(fix)).build(),
    }
}

/// Baut einen Behebungsvorschlag.
fn build_fix(spec: &FixSpec) -> humanitl_core::FixAction {
    use humanitl_core::FixAction;
    match spec {
        FixSpec::SetEnv { key, value } => FixAction::SetEnv {
            key: key.clone(),
            value: value.clone(),
        },
        FixSpec::ChangeSetting { key, value } => FixAction::ChangeSetting {
            key: key.clone(),
            value: value.clone(),
        },
        FixSpec::CopyCommand(command) => FixAction::CopyCommand(command.clone()),
        FixSpec::OpenUrl(url) => FixAction::OpenUrl(url.clone()),
    }
}

/// Baut einen Fund aus seiner Beschreibung.
fn build_finding(spec: &FindingSpec) -> Finding {
    let location = match spec.location.split_once(':') {
        Some(("header", name)) => HeaderName::from_bytes(name.as_bytes())
            .map_or(FindingLocation::Body, FindingLocation::Header),
        _ if spec.location == "query" => FindingLocation::Query,
        _ => FindingLocation::Body,
    };
    let tier = match spec.tier.as_deref() {
        Some("regex") => Tier::Regex,
        Some("user_term") => Tier::UserTerm,
        _ => Tier::Checksum,
    };
    let span = spec
        .span
        .map_or_else(|| 0..spec.value.len(), |(start, end)| start..end);
    Finding::new(finding_kind(&spec.kind), span, location, tier, &spec.value)
}

/// Liest eine Fundart aus ihrem Wire-Namen.
fn finding_kind(text: &str) -> FindingKind {
    match text.split_once('.') {
        Some(("api_key", name)) => FindingKind::ApiKey(name.to_owned()),
        Some(("user_term", name)) => FindingKind::UserTerm(name.to_owned()),
        Some(("custom", name)) => FindingKind::Custom(name.to_owned()),
        _ => match text {
            "jwt" => FindingKind::Jwt,
            "email" => FindingKind::Email,
            "iban" => FindingKind::Iban,
            "credit_card" => FindingKind::CreditCard,
            "phone" => FindingKind::Phone,
            "ipv4" => FindingKind::Ipv4,
            other => FindingKind::Custom(other.to_owned()),
        },
    }
}

/// Liest eine Methode; eine unbekannte wird `GET`.
fn method_of(text: &str) -> Method {
    Method::from_bytes(text.as_bytes()).unwrap_or(Method::GET)
}

/// Liest ein Schema mit Rückfallwert.
fn scheme_of(text: Option<&str>, fallback: Scheme) -> Scheme {
    text.and_then(Scheme::parse).unwrap_or(fallback)
}

/// Baut ein Ziel; ein unlesbarer Host wird zu `invalid.example`.
fn authority_of(host: &str, port: Option<u16>, scheme: Scheme) -> Authority {
    let host =
        HostName::parse(host).unwrap_or_else(|_| HostName::Dns("invalid.example".to_owned()));
    Authority::new(host, port.unwrap_or_else(|| scheme.default_port()))
}

/// Baut Kopfzeilen aus Paaren; unlesbare fallen weg.
fn header_map(pairs: &[(String, String)]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for (name, value) in pairs {
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(value) else {
            continue;
        };
        headers.append(name, value);
    }
    headers
}

/// Setzt eine Kopfzeile, wenn sie noch fehlt.
fn add_missing(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    let name = HeaderName::from_static(name);
    if !headers.contains_key(&name) {
        headers.insert(name, HeaderValue::from_static(value));
    }
}

/// Baut einen Body aus Base64 oder Text.
///
/// `body_b64` hat Vorrang; `body` ist die bequeme Schreibweise für
/// handgeschriebene Fixtures. Ungültiges Base64 weist [`Session::parse`]
/// schon beim Lesen ab ([`check_body_b64`]); hier kommt nur an, was dort
/// durch war.
fn body_of(body_b64: Option<&str>, body: Option<&str>, headers: &HeaderMap) -> BodyRef {
    let bytes = match (body_b64, body) {
        (Some(encoded), _) if !encoded.is_empty() => match decode_body_b64(encoded) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!(%error, "body_b64 was not checked at parse time; body is empty");
                Bytes::new()
            }
        },
        (_, Some(text)) => Bytes::from(text.to_owned()),
        _ => Bytes::new(),
    };
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let mut body = BodyRef::from_bytes(bytes);
    body.content_type = content_type;
    body
}

/// Baut einen Body aus reinem Text.
fn text_body(text: Option<&str>, content_type: Option<&str>) -> BodyRef {
    let mut body = BodyRef::from_bytes(Bytes::from(text.unwrap_or_default().to_owned()));
    body.content_type = content_type
        .map(ToOwned::to_owned)
        .or_else(|| Some("application/json".to_owned()));
    body
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, SystemTime};

    use super::{Pass, PassStamps, PlayerOptions, Session, SessionError, flow_id};

    const LINES: &str = r#"
{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001"}
{"t_ms":120,"type":"request","flow_id":"018f0000-0000-7000-8000-000000010000","method":"GET","host":"registry.npmjs.org","path":"/lodash"}
{"t_ms":10,"type":"hold","flow_id":"018f0000-0000-7000-8000-000000010000"}
"#;

    #[test]
    fn parse_sorts_by_time_and_skips_comments() {
        let session = Session::parse(LINES).expect("parses");
        assert_eq!(session.lines().len(), 3);
        assert_eq!(session.lines()[0].t_ms, 0);
        assert_eq!(session.lines()[1].t_ms, 10);
        assert_eq!(session.span_ms(), 120);
        assert!(session.meta().is_some());
    }

    #[test]
    fn parse_rejects_an_unknown_diagnostic_code() {
        let text = r#"{"t_ms":0,"type":"diagnostic","code":"NOPE_001","why":"x"}"#;
        let err = Session::parse(text).expect_err("must fail");
        assert!(err.to_string().contains("NOPE_001"), "{err}");
    }

    #[test]
    fn parse_rejects_a_broken_flow_id() {
        let text = r#"{"t_ms":0,"type":"hold","flow_id":"nope"}"#;
        let err = Session::parse(text).expect_err("must fail");
        assert!(err.to_string().contains("not a uuid"), "{err}");
    }

    /// Eine Zeile mit kaputtem Base64 hält die Datei beim Lesen an und nennt
    /// Zeile und Feld; früher wurde daraus still ein leerer Body.
    #[test]
    fn parse_rejects_malformed_body_b64_naming_line_and_field() {
        let lines = [
            (
                "request",
                r#"{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001"}
{"t_ms":10,"type":"request","flow_id":"018f0000-0000-7000-8000-000000010000","method":"POST","host":"api.example.org","path":"/","body_b64":"not*base64!"}"#,
            ),
            (
                "response",
                r#"{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001"}
{"t_ms":10,"type":"response","flow_id":"018f0000-0000-7000-8000-000000010000","status":200,"body_b64":"e30"}"#,
            ),
            (
                "passthrough",
                r#"{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001"}
{"t_ms":10,"type":"passthrough","flow_id":"018f0000-0000-7000-8000-000000010000","method":"POST","host":"127.0.0.1","path":"/api/chat","body_b64":"====","response_body":"{}"}"#,
            ),
        ];
        for (kind, text) in lines {
            let err = Session::parse(text).expect_err(kind);
            let SessionError::Field { line, ref reason } = err else {
                panic!("{kind}: expected a field error, got {err:?}");
            };
            assert_eq!(line, 2, "{kind}: the second line carries the body");
            assert!(
                reason.starts_with("body_b64 is not valid base64"),
                "{kind}: {reason}"
            );
            assert!(
                err.to_string().contains("session line 2: body_b64"),
                "{kind}: {err}"
            );
            assert_eq!(err.diagnostic().code.as_str(), "CONFIG_001");
        }
    }

    #[test]
    fn parse_accepts_valid_and_empty_body_b64() {
        let text = r#"{"t_ms":0,"type":"session","session_id":"018f0000-0000-7000-8000-000000000001"}
{"t_ms":10,"type":"request","flow_id":"018f0000-0000-7000-8000-000000010000","method":"POST","host":"api.example.org","path":"/","body_b64":"e30="}
{"t_ms":20,"type":"response","flow_id":"018f0000-0000-7000-8000-000000010000","status":200,"body_b64":"","body":"{}"}"#;
        let session = Session::parse(text).expect("valid base64 and an empty body_b64 pass");
        assert_eq!(session.lines().len(), 3);
    }

    #[test]
    fn speed_divides_offsets_but_not_timeouts() {
        let options = PlayerOptions {
            speed: 10.0,
            ..PlayerOptions::default()
        };
        assert_eq!(options.offset(20_000), Duration::from_secs(2));
        assert_eq!(options.hold_span(Some(5_000)), Duration::from_secs(5));

        let scaled = PlayerOptions {
            speed: 10.0,
            scale_timeouts: true,
            ..PlayerOptions::default()
        };
        assert_eq!(scaled.hold_span(Some(5_000)), Duration::from_millis(500));
    }

    #[test]
    fn a_speed_without_meaning_leaves_the_span_alone() {
        for speed in [
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -2.0,
            1e-300,
        ] {
            let options = PlayerOptions {
                speed,
                ..PlayerOptions::default()
            };
            assert_eq!(
                options.scaled(Duration::from_millis(500)),
                Duration::from_millis(500),
                "speed {speed} must not panic and must not scale"
            );
        }
    }

    #[test]
    fn looping_keeps_the_tail_and_moves_the_timestamp() {
        let text = "018f0000-0000-7000-8000-0000000abcde";
        let mut stamps = PassStamps::default();
        let first = flow_id(text, stamps.next(0, SystemTime::now()));
        let second = flow_id(text, stamps.next(1, SystemTime::now()));
        assert_eq!(first.to_string(), text);
        assert_ne!(first, second);
        assert_eq!(
            first.as_uuid().as_bytes()[6..],
            second.as_uuid().as_bytes()[6..]
        );
        assert!(second > first, "{second:?} must sort after {first:?}");
    }

    /// Jede Zeile eines Durchlaufs rechnet ihre Id vom selben Zeitpunkt aus;
    /// die Uhr beim Lesen der Zeile spielt keine Rolle.
    #[test]
    fn every_id_of_a_pass_derives_from_the_start_of_that_pass() {
        let text = "018f0000-0000-7000-8000-0000000abcde";
        let pass = Pass {
            iteration: 2,
            stamp_ms: 0x0123_4567_89ab,
        };

        let request = flow_id(text, pass);
        let hold = flow_id(text, pass);
        assert_eq!(request, hold, "the request line and the hold line agree");

        let uuid = request.as_uuid();
        assert_eq!(uuid.as_bytes()[0..6], [0x01, 0x23, 0x45, 0x67, 0x89, 0xab]);
        assert_eq!(
            pass.started_at(),
            SystemTime::UNIX_EPOCH + Duration::from_millis(0x0123_4567_89ab)
        );

        let later = flow_id(
            text,
            Pass {
                iteration: 3,
                stamp_ms: 0x0123_4567_89ac,
            },
        );
        assert_ne!(request, later, "the next pass gets its own ids");
        assert!(later > request);
    }

    /// Zwei Durchläufe in derselben Wanduhr-Millisekunde bekommen zwei
    /// Stempel, der zweite eine Millisekunde später, und damit zwei Ids. Eine
    /// rückwärts gestellte Uhr vergibt keinen Stempel ein zweites Mal.
    #[test]
    fn passes_in_the_same_millisecond_get_distinct_increasing_stamps() {
        let text = "018f0000-0000-7000-8000-0000000abcde";
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        let mut stamps = PassStamps::default();

        let first = stamps.next(1, now);
        let second = stamps.next(2, now);
        assert_eq!(first.stamp_ms, 1_700_000_000_000);
        assert_eq!(
            second.stamp_ms,
            first.stamp_ms + 1,
            "same millisecond, next stamp"
        );

        let first_id = flow_id(text, first);
        let second_id = flow_id(text, second);
        assert_ne!(first_id, second_id, "the two passes must not share ids");
        assert!(
            second_id > first_id,
            "{second_id:?} must sort after {first_id:?}"
        );
        assert_eq!(
            first_id.as_uuid().as_bytes()[6..],
            second_id.as_uuid().as_bytes()[6..],
            "only the time changes"
        );

        let earlier = stamps.next(3, now - Duration::from_secs(5));
        assert_eq!(
            earlier.stamp_ms,
            second.stamp_ms + 1,
            "a clock set back is not trusted"
        );

        let later = stamps.next(4, now + Duration::from_secs(5));
        assert_eq!(
            later.stamp_ms, 1_700_000_005_000,
            "a clock that moved on is used as is"
        );
    }
}
