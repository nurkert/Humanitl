//! Die Abbildung zwischen den Kern-Typen und der Wire-Form aus [`crate::v1`].
//!
//! Hier steht jede Übersetzung genau einmal. Der Fake ([`crate::fake`]) und der
//! echte Dienst ([`crate::server`]) benutzen dieselben Funktionen; wo beide
//! dasselbe Feld füllen, kann es nicht auseinanderlaufen, und die Oberfläche
//! sieht zwischen Fake und Daemon keinen Unterschied (HUM-005, HUM-018).
//!
//! Zwei Regeln des Vertrags gelten hier und sind der Grund, warum die
//! Übersetzung eine eigene Datei ist:
//!
//! - Bodies reisen nie in einem Ereignis, nur als [`v1::BodyRef`]
//!   (`backlog/CONVENTIONS.md` 3.6). Der einzige Body in Richtung Daemon ist
//!   [`v1::EditedRequest`], und der wird in [`request_from_proto`] gelesen.
//! - Ein Enum mit `_UNSPECIFIED = 0` wird nie geraten. Wo die Gegenrichtung
//!   ein Enum liest, ist `Unspecified` ein Fehler ([`EditedRequestError`]),
//!   nie stillschweigend ein Vorgabewert.
//!
//! Fristen sind im Kern ein [`Instant`] (die Uhr, an der die Warteschlange
//! hängt), im Vertrag ein `Timestamp`. [`wall_clock`] rechnet um; die
//! Umrechnung geschieht beim Bauen des Ereignisses, damit die Frist, die die
//! Oberfläche anzeigt, dieselbe ist, die die Warteschlange meint
//! (`backlog/sprint-1.md`, HUM-019).

use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use humanitl_core::http::{
    Authority, BodyRef, HeaderMap, HeaderName, HeaderValue, HttpRequest, Method, Scheme,
};
use humanitl_core::rule::{Expiry, Rule};
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Diagnostic, Finding, FixAction, FlowEvent, FlowId,
    FlowState, HostName, RuleId, SessionId, Severity, UpstreamError, sanitize_note,
};
use humanitl_proxy::registry::{FlowRecord, FlowRegistry};
use humanitl_proxy::rules_store::StoredRule;
use humanitl_proxy::{LlmFlavor, ProbeResult};
use humanitl_recorder::{
    Dir, FindingRecord, FlowDetail as RecordedDetail, FlowSummary as RecordedSummary, MessageRecord,
};
use humanitl_sandbox::{CheckResult, IsolationCheck};

use crate::domains::DomainTable;
use crate::v1;

/// Die Protokollfassung, die der Proxy in M1 beidseitig spricht (ADR-016).
pub const HTTP_VERSION: &str = "HTTP/1.1";

/// So viele Zeichen trägt `FlowDetail.body_preview` höchstens (docs/PROTOCOL.md 4).
pub const BODY_PREVIEW_CHARS: usize = 4096;

/// Warum eine `EditedRequest` nicht lesbar war.
///
/// Der Daemon lehnt eine unlesbare Anfrage mit `IPC_004` ab, statt sie
/// stillschweigend zu `Allow` zu machen; der Grund steht im Diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum EditedRequestError {
    /// `METHOD_UNSPECIFIED`, `METHOD_OTHER` ohne `method_raw`, oder der
    /// Rohwert ist kein HTTP-Token.
    #[error("the method is not a valid http method")]
    Method,
    /// Die URL hat kein bekanntes Schema (`http`, `https`, `ws`, `wss`).
    #[error("the url has no known scheme")]
    Scheme,
    /// Der Host ist leer, kein gültiger Name und keine IP-Adresse.
    #[error("the url has no valid host")]
    Host,
    /// Der Port ist keine Zahl von 1 bis 65535.
    #[error("the url has no valid port")]
    Port,
    /// Die URL trägt ein Fragment oder Userinfo; beides ist kein Request-Ziel,
    /// und der Fake ändert nicht stillschweigend, was der Mensch geschrieben hat.
    #[error("the url carries a fragment or userinfo")]
    Target,
}

/// Übersetzt einen Befund in seine Wire-Form.
///
/// `why` geht durch [`sanitize_note`]. Es ist der einzige Teil eines Befunds,
/// den der Daemon zur Laufzeit zusammensetzt, und in mehr als einem Fall
/// steht darin Text, den nicht er geschrieben hat: der Socket-Dateiname aus
/// `/work` in `SANDBOX_015`, ein Hostname, ein Pfad. Er landet ungefiltert in
/// einer Karte der Oberfläche, und die pinnt keine Textrichtung — eine
/// Rechts-nach-links-Marke stellte den Satz um, mit dem ein Mensch über seine
/// Sandbox entscheidet. Dieselbe Säuberung wie in [`check_result_to_proto`]
/// und in einer Blockier-Notiz: eine zweite Regel daneben liefe von ihr weg.
#[must_use]
pub fn diagnostic_to_proto(diagnostic: &Diagnostic) -> v1::Diagnostic {
    v1::Diagnostic {
        code: diagnostic.code.as_str().to_owned(),
        severity: severity_to_proto(diagnostic.severity) as i32,
        title: diagnostic.title.clone(),
        why: sanitize_note(&diagnostic.why),
        fix: diagnostic.fix.as_ref().map(fix_to_proto),
        docs_url: diagnostic.docs.clone().unwrap_or_default(),
    }
}

/// Übersetzt eine Dringlichkeit in ihre Wire-Form.
#[must_use]
pub const fn severity_to_proto(severity: Severity) -> v1::Severity {
    match severity {
        Severity::Info => v1::Severity::Info,
        Severity::Warning => v1::Severity::Warning,
        Severity::Error => v1::Severity::Error,
        Severity::Blocking => v1::Severity::Blocking,
    }
}

/// Übersetzt einen Behebungsvorschlag in seine Wire-Form.
#[must_use]
pub fn fix_to_proto(fix: &FixAction) -> v1::FixAction {
    use v1::fix_action::Action;

    let action = match fix {
        FixAction::SetEnv { key, value } => Action::SetEnv(v1::fix_action::SetEnv {
            key: key.clone(),
            value: value.clone(),
        }),
        FixAction::AddRule(rule) => Action::AddRule(rule_to_proto(rule)),
        FixAction::InstallService => Action::InstallService(()),
        FixAction::ChangeSetting { key, value } => {
            Action::ChangeSetting(v1::fix_action::ChangeSetting {
                key: key.clone(),
                value: value.clone(),
            })
        }
        FixAction::CopyCommand(command) => Action::CopyCommand(command.clone()),
        FixAction::OpenUrl(url) => Action::OpenUrl(url.clone()),
        FixAction::RemountReadOnly(path) => {
            Action::RemountReadOnly(path.to_string_lossy().into_owned())
        }
    };
    v1::FixAction {
        action: Some(action),
    }
}

/// Übersetzt eine Isolationsprüfung in ihre Wire-Form.
#[must_use]
pub const fn isolation_check_to_proto(check: IsolationCheck) -> v1::IsolationCheck {
    match check {
        IsolationCheck::NoNetworkInterface => v1::IsolationCheck::NoNetworkInterface,
        IsolationCheck::SingleSocket => v1::IsolationCheck::SingleSocket,
        IsolationCheck::SeccompActive => v1::IsolationCheck::SeccompActive,
    }
}

/// Übersetzt das Ergebnis einer Isolationsprüfung in seine Wire-Form.
///
/// Die Evidenz ist die eine Stelle dieses Ereignisses, an der Text steht, den
/// nicht der Daemon geschrieben hat. Der Suchlauf des Shims läuft bis Tiefe 3
/// auch über `/work`, und ein Socket-Dateiname dort stammt vom Agenten; er
/// landet als Text neben einem Punkt genau dort, wo ein Mensch entscheidet,
/// ob er der Sandbox glaubt. Die Säuberung des Shims
/// (`humanitl_sandbox::report`) ersetzt nur Whitespace und Steuerzeichen, und
/// `parse_check_line` prüft nichts nach; eine Rechts-nach-links-Marke käme so
/// bis in die Oberfläche und stellte die Zeile um, die sie belegen soll.
///
/// [`sanitize_note`] ist dieselbe Säuberung, die eine Blockier-Notiz nimmt:
/// Steuerzeichen, unsichtbare Zeichen und Bidi-Marken fallen weg, Whitespace
/// wird zusammengezogen, gestapelte kombinierende Zeichen werden begrenzt, und
/// die Länge ist auf [`humanitl_core::block::NOTE_MAX_CHARS`] gedeckelt. Eine zweite Fassung
/// derselben Regel daneben liefe von ihr weg.
#[must_use]
pub fn check_result_to_proto(result: &CheckResult) -> v1::CheckResult {
    v1::CheckResult {
        check: isolation_check_to_proto(result.check) as i32,
        passed: result.passed,
        evidence: sanitize_note(&result.evidence),
        diagnostic: result.diagnostic.as_ref().map(diagnostic_to_proto),
    }
}

/// Übersetzt die Aktion einer Regel in ihre Wire-Form.
#[must_use]
pub const fn action_to_proto(action: humanitl_core::rule::Action) -> v1::RuleAction {
    match action {
        humanitl_core::rule::Action::Allow => v1::RuleAction::Allow,
        humanitl_core::rule::Action::Block => v1::RuleAction::Block,
        humanitl_core::rule::Action::Ask => v1::RuleAction::Ask,
        humanitl_core::rule::Action::Redact => v1::RuleAction::Redact,
    }
}

/// Übersetzt das Ergebnis der Endpunkt-Probe in seine Wire-Form (HUM-039).
///
/// `diagnostic` trägt den ersten Befund noch einmal, weil ein Client, der nur
/// eine Karte zeigt, sonst raten müsste, welcher gemeint ist; `diagnostics`
/// trägt alle. Ein leeres `models` bei `flavor = UNKNOWN` ist eine Aussage:
/// Es hat sich nichts gemeldet, was Humanitl kennt.
#[must_use]
pub fn probe_result_to_proto(result: &ProbeResult) -> v1::ProbeLlmResponse {
    let diagnostics: Vec<v1::Diagnostic> =
        result.diagnostics.iter().map(diagnostic_to_proto).collect();
    v1::ProbeLlmResponse {
        models: result.models.clone(),
        flavor: llm_flavor_to_proto(result.flavor) as i32,
        diagnostic: diagnostics.first().cloned(),
        latency_ms: result.latency_ms,
        diagnostics,
        endpoint_is_private: result.endpoint_is_private,
    }
}

/// Übersetzt die erkannte API in ihre Wire-Form.
const fn llm_flavor_to_proto(flavor: LlmFlavor) -> v1::LlmProduct {
    match flavor {
        LlmFlavor::Ollama => v1::LlmProduct::Ollama,
        LlmFlavor::OpenAiCompatible => v1::LlmProduct::OpenaiCompatible,
        LlmFlavor::Unknown => v1::LlmProduct::Unknown,
    }
}

/// Übersetzt eine Regel in ihre Wire-Form.
#[must_use]
pub fn rule_to_proto(rule: &Rule) -> v1::Rule {
    let action = action_to_proto(rule.action);
    v1::Rule {
        rule_id: rule.id.to_string(),
        action: action as i32,
        matcher: Some(matcher_to_proto(&rule.matcher)),
        expires: Some(expiry_to_proto(rule.expires)),
        stream: rule.stream,
        created_from_flow_id: rule
            .created_from
            .map_or_else(String::new, |id| id.to_string()),
        bundled: rule.bundled,
        note: rule.note.clone().unwrap_or_default(),
        created_at: None,
        position: 0,
        hit_count: 0,
        allow_private: rule.allow_private,
        disabled: rule.disabled,
        passthrough_llm: rule.passthrough_llm,
    }
}

/// Übersetzt die Bedingung einer Regel in ihre Wire-Form.
fn matcher_to_proto(matcher: &humanitl_core::rule::Matcher) -> v1::RuleMatcher {
    v1::RuleMatcher {
        host: matcher.host.to_string(),
        methods: matcher
            .methods
            .as_ref()
            .map(|methods| methods.iter().map(|m| method_to_proto(m) as i32).collect())
            .unwrap_or_default(),
        path: matcher
            .path
            .as_ref()
            .map_or_else(String::new, ToString::to_string),
        scheme: matcher
            .scheme
            .map_or(0, |scheme| scheme_to_proto(scheme) as i32),
        port: u32::from(matcher.port.unwrap_or(0)),
        path_prefixes: matcher.path_prefixes.clone(),
        upgrade: match matcher.upgrade {
            Some(humanitl_core::http::Upgrade::WebSocket) => v1::Upgrade::Websocket as i32,
            None => v1::Upgrade::None as i32,
        },
    }
}

/// Übersetzt die Gültigkeit einer Regel in ihre Wire-Form.
fn expiry_to_proto(expiry: Expiry) -> v1::RuleExpiry {
    let expiry = match expiry {
        Expiry::Never => v1::rule_expiry::Expiry::Never(()),
        Expiry::Session(_) => v1::rule_expiry::Expiry::Session(()),
        Expiry::At(at) => v1::rule_expiry::Expiry::At(prost_types::Timestamp {
            seconds: at.timestamp(),
            nanos: i32::try_from(at.timestamp_subsec_nanos().min(999_999_999)).unwrap_or(0),
        }),
    };
    v1::RuleExpiry {
        expiry: Some(expiry),
    }
}

/// Übersetzt eine HTTP-Methode in ihre Wire-Form.
#[must_use]
pub fn method_to_proto(method: &humanitl_core::http::Method) -> v1::Method {
    match *method {
        humanitl_core::http::Method::GET => v1::Method::Get,
        humanitl_core::http::Method::HEAD => v1::Method::Head,
        humanitl_core::http::Method::POST => v1::Method::Post,
        humanitl_core::http::Method::PUT => v1::Method::Put,
        humanitl_core::http::Method::PATCH => v1::Method::Patch,
        humanitl_core::http::Method::DELETE => v1::Method::Delete,
        humanitl_core::http::Method::OPTIONS => v1::Method::Options,
        humanitl_core::http::Method::CONNECT => v1::Method::Connect,
        humanitl_core::http::Method::TRACE => v1::Method::Trace,
        _ => v1::Method::Other,
    }
}

/// Übersetzt ein Schema in seine Wire-Form.
#[must_use]
pub const fn scheme_to_proto(scheme: humanitl_core::http::Scheme) -> v1::Scheme {
    match scheme {
        humanitl_core::http::Scheme::Http => v1::Scheme::Http,
        humanitl_core::http::Scheme::Https => v1::Scheme::Https,
        humanitl_core::http::Scheme::Ws => v1::Scheme::Ws,
        humanitl_core::http::Scheme::Wss => v1::Scheme::Wss,
    }
}

/// Das Ereignis, mit dem ein verpasster Rundfunk-Abschnitt gemeldet wird.
#[must_use]
pub fn lagged_event(dropped: u64) -> v1::FlowEvent {
    v1::FlowEvent {
        at: Some(timestamp(SystemTime::now())),
        event: Some(v1::flow_event::Event::Lagged(v1::flow_event::Lagged {
            dropped,
        })),
    }
}

/// Entscheidung, Blockgrund und Regel-Id in ihrer Wire-Form.
pub(crate) fn decision_fields(
    decision: Option<&Decision>,
    source: Option<DecisionSource>,
) -> (v1::DecisionKind, v1::BlockReason, String) {
    let rule_id = match (decision, source) {
        (
            Some(Decision::Block {
                reason: BlockReason::Rule(id),
                ..
            }),
            _,
        ) => id.to_string(),
        (_, Some(DecisionSource::Rule(id))) => id.to_string(),
        _ => String::new(),
    };
    let kind = match decision {
        None => v1::DecisionKind::Unspecified,
        Some(Decision::Allow) => v1::DecisionKind::Allow,
        Some(Decision::AllowEdited { .. }) => v1::DecisionKind::AllowEdited,
        Some(Decision::Block { .. }) => v1::DecisionKind::Block,
        Some(Decision::TimedOut) => v1::DecisionKind::TimedOut,
    };
    let reason = match decision {
        Some(Decision::Block { reason, .. }) => block_reason_to_proto(*reason),
        Some(Decision::TimedOut) => v1::BlockReason::Timeout,
        _ => v1::BlockReason::Unspecified,
    };
    (kind, reason, rule_id)
}

/// Die Notiz einer Block-Entscheidung, sonst leer.
pub(crate) fn block_note(decision: &Decision) -> String {
    match decision {
        Decision::Block { note, .. } => note.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

/// Übersetzt einen Blockgrund in seine Wire-Form.
#[must_use]
pub const fn block_reason_to_proto(reason: BlockReason) -> v1::BlockReason {
    use humanitl_core::BlockReason as Reason;
    match reason {
        Reason::User => v1::BlockReason::User,
        Reason::Rule(_) => v1::BlockReason::Rule,
        Reason::Timeout => v1::BlockReason::Timeout,
        Reason::BodyCap => v1::BlockReason::BodyCap,
        Reason::AuthorityMismatch => v1::BlockReason::AuthorityMismatch,
        Reason::NoRoute => v1::BlockReason::NoRoute,
        Reason::HoldMemory => v1::BlockReason::HoldMemory,
        Reason::HoldMaxFlows => v1::BlockReason::HoldMaxFlows,
        Reason::ClientTimeout => v1::BlockReason::ClientTimeout,
        Reason::PrivateAddress => v1::BlockReason::PrivateAddress,
        Reason::Secret => v1::BlockReason::Secret,
    }
}

/// Übersetzt die Herkunft einer Entscheidung in ihre Wire-Form.
#[must_use]
pub const fn source_to_proto(source: DecisionSource) -> v1::DecisionSource {
    match source {
        DecisionSource::User => v1::DecisionSource::User,
        DecisionSource::Rule(_) => v1::DecisionSource::Rule,
        DecisionSource::Timeout => v1::DecisionSource::Timeout,
        DecisionSource::Passthrough => v1::DecisionSource::Passthrough,
        DecisionSource::System => v1::DecisionSource::System,
    }
}

/// Übersetzt einen Upstream-Fehler in seine Wire-Form.
#[must_use]
pub const fn upstream_error_to_proto(error: UpstreamError) -> v1::UpstreamError {
    match error {
        UpstreamError::Dns => v1::UpstreamError::Dns,
        UpstreamError::Connect => v1::UpstreamError::Connect,
        UpstreamError::Tls => v1::UpstreamError::Tls,
        UpstreamError::PrivateAddress(_) => v1::UpstreamError::PrivateAddress,
        UpstreamError::Timeout => v1::UpstreamError::Timeout,
    }
}

/// Übersetzt einen Flow-Zustand in seine Wire-Form.
#[must_use]
pub const fn flow_state_to_proto(state: &FlowState) -> v1::FlowState {
    match state {
        FlowState::Received => v1::FlowState::Received,
        FlowState::Analyzed { .. } => v1::FlowState::Analyzed,
        FlowState::Held { .. } => v1::FlowState::Held,
        FlowState::Decided(_) => v1::FlowState::Decided,
        FlowState::Forwarded => v1::FlowState::Forwarded,
        FlowState::Responded { .. } => v1::FlowState::Responded,
        FlowState::Failed { .. } => v1::FlowState::Failed,
        FlowState::Recorded => v1::FlowState::Recorded,
    }
}

/// Der Rohwert einer Methode, aber nur bei einer unbekannten.
pub(crate) fn method_raw(method: &Method) -> String {
    if method_to_proto(method) == v1::Method::Other {
        method.as_str().to_owned()
    } else {
        String::new()
    }
}

/// Übersetzt ein Ziel in seine Wire-Form.
#[must_use]
pub fn authority_to_proto(authority: &Authority) -> v1::Authority {
    v1::Authority {
        host: authority.host.to_string(),
        port: u32::from(authority.port),
        is_ip_literal: matches!(authority.host, HostName::Ip(_)),
        display_host: authority.host.display(),
    }
}

/// Übersetzt Kopfzeilen in ihre Wire-Form.
#[must_use]
pub fn headers_to_proto(headers: &HeaderMap) -> Vec<v1::Header> {
    headers
        .iter()
        .map(|(name, value)| v1::Header {
            name: name.as_str().to_owned(),
            value: value.as_bytes().to_vec(),
        })
        .collect()
}

/// Liest Kopfzeilen aus ihrer Wire-Form.
///
/// Eine Kopfzeile, deren Name oder Wert HTTP nicht erlaubt, fällt weg; an
/// einer kaputten Eingabe stirbt der Daemon nicht.
#[must_use]
pub fn headers_from_proto(headers: &[v1::Header]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for header in headers {
        let Ok(name) = HeaderName::from_bytes(header.name.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_bytes(&header.value) else {
            continue;
        };
        map.append(name, value);
    }
    map
}

/// Übersetzt einen Body-Verweis in seine Wire-Form.
#[must_use]
pub fn body_to_proto(body: &BodyRef) -> v1::BodyRef {
    v1::BodyRef {
        sha256: body.sha256.to_vec(),
        size: body.size,
        truncated: body.truncated,
        content_type: body.content_type.clone().unwrap_or_default(),
    }
}

/// Übersetzt eine Anfrage in ihre Wire-Form.
#[must_use]
pub fn request_to_proto(request: &HttpRequest) -> v1::HttpRequest {
    v1::HttpRequest {
        method: method_to_proto(&request.method) as i32,
        method_raw: method_raw(&request.method),
        scheme: scheme_to_proto(request.scheme) as i32,
        authority: Some(authority_to_proto(&request.authority)),
        path_and_query: request.path_and_query.clone(),
        headers: headers_to_proto(&request.headers),
        body: Some(body_to_proto(&request.body)),
        version: HTTP_VERSION.to_owned(),
    }
}

/// Liest die bearbeitete Anfrage aus ihrer Wire-Form.
///
/// Anders als `HttpRequest` trägt `EditedRequest` den Body selbst; er bleibt
/// inline, damit der Aufrufer ihn weiterreichen oder ablegen kann.
/// Die URL wird ohne die `url`-Crate zerlegt: `scheme://host[:port]/path?query`.
/// Fehlt der Port, gilt der des Schemas; fehlt der Pfad, gilt `/`. Der
/// `Content-Type` des Bodys kommt aus den Kopfzeilen.
///
/// # Errors
///
/// [`EditedRequestError`] nennt, was unlesbar war. Der Aufrufer lehnt dann
/// ab; er macht aus der Anfrage nie ein `Allow`.
pub fn request_from_proto(request: &v1::EditedRequest) -> Result<HttpRequest, EditedRequestError> {
    let method = method_from_proto(request.method, &request.method_raw)?;
    let (scheme, authority, path_and_query) = split_url(&request.url)?;
    let headers = headers_from_proto(&request.headers);
    let content_type = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let mut body = BodyRef::from_bytes(Bytes::copy_from_slice(&request.body));
    body.content_type = content_type;
    Ok(HttpRequest::new(method, scheme, authority, path_and_query)
        .with_headers(headers)
        .with_body(body))
}

/// Die Methode aus Enum und Rohwert. Ein gesetzter Rohwert gilt.
/// Liest eine Methode von der Leitung.
///
/// `METHOD_UNSPECIFIED` und ein `METHOD_OTHER` ohne Rohwert sind ein Fehler:
/// Eine Methode, die der Daemon nicht kennt, wird nicht zu `GET` ergänzt.
///
/// # Errors
///
/// [`EditedRequestError::Method`], wenn die Methode fehlt oder kein
/// HTTP-Token ist.
pub fn method_from_proto(method: i32, raw: &str) -> Result<Method, EditedRequestError> {
    let text = if raw.is_empty() {
        match v1::Method::try_from(method) {
            Ok(v1::Method::Unspecified | v1::Method::Other) | Err(_) => {
                return Err(EditedRequestError::Method);
            }
            Ok(known) => known.as_str_name().trim_start_matches("METHOD_").to_owned(),
        }
    } else {
        raw.to_owned()
    };
    Method::from_bytes(text.as_bytes()).map_err(|_| EditedRequestError::Method)
}

/// Zerlegt `scheme://host[:port]/path?query` in Schema, Ziel und Pfad.
///
/// # Errors
///
/// [`EditedRequestError`] mit der Stelle, an der die URL nicht las: Schema,
/// Host, Port oder ein Fragment beziehungsweise Userinfo, das in einem
/// Request-Ziel nichts zu suchen hat.
pub fn split_url(url: &str) -> Result<(Scheme, Authority, String), EditedRequestError> {
    let (scheme, rest) = url.split_once("://").ok_or(EditedRequestError::Scheme)?;
    let scheme = Scheme::parse(scheme).ok_or(EditedRequestError::Scheme)?;
    if rest.contains('#') {
        return Err(EditedRequestError::Target);
    }
    let (authority, path) = match rest.find(['/', '?']) {
        Some(index) => (&rest[..index], &rest[index..]),
        None => (rest, "/"),
    };
    let path_and_query = if path.starts_with('?') {
        format!("/{path}")
    } else {
        path.to_owned()
    };
    if authority.contains('@') {
        return Err(EditedRequestError::Target);
    }
    let (host, port) = split_host_port(authority)?;
    let host = HostName::parse(host).map_err(|_| EditedRequestError::Host)?;
    let port = match port {
        Some(text) => text
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(EditedRequestError::Port)?,
        None => scheme.default_port(),
    };
    Ok((scheme, Authority::new(host, port), path_and_query))
}

/// `host`, `host:port`, `[v6]`, `[v6]:port`.
fn split_host_port(authority: &str) -> Result<(&str, Option<&str>), EditedRequestError> {
    if authority.is_empty() {
        return Err(EditedRequestError::Host);
    }
    if authority.starts_with('[') {
        let end = authority.find(']').ok_or(EditedRequestError::Host)?;
        let (host, after) = authority.split_at(end + 1);
        return match after.strip_prefix(':') {
            Some(port) => Ok((host, Some(port))),
            None if after.is_empty() => Ok((host, None)),
            None => Err(EditedRequestError::Host),
        };
    }
    Ok(authority
        .rsplit_once(':')
        .map_or((authority, None), |(host, port)| (host, Some(port))))
}

/// Die Vorschau eines Bodys für `FlowDetail.body_preview`: die ersten
/// [`BODY_PREVIEW_CHARS`] Zeichen als UTF-8, ungültige Bytes als U+FFFD.
///
/// Gelesen werden höchstens `4 * BODY_PREVIEW_CHARS` Bytes, denn länger sind
/// so viele Zeichen in UTF-8 nie. Ein am Schnitt zerteiltes Zeichen läge
/// damit immer hinter dem letzten gezeigten und fällt weg.
#[must_use]
pub fn body_preview(body: &[u8]) -> String {
    let scan = body.len().min(BODY_PREVIEW_CHARS * 4);
    String::from_utf8_lossy(&body[..scan])
        .chars()
        .take(BODY_PREVIEW_CHARS)
        .collect()
}

/// Übersetzt einen Fund in seine Wire-Form.
#[must_use]
pub fn finding_to_proto(finding: &Finding) -> v1::Finding {
    let (location, header_name) = match &finding.location {
        humanitl_core::FindingLocation::Header(name) => {
            (v1::FindingLocation::Header, name.as_str().to_owned())
        }
        humanitl_core::FindingLocation::Query => (v1::FindingLocation::Query, String::new()),
        humanitl_core::FindingLocation::Body => (v1::FindingLocation::Body, String::new()),
    };
    let tier = match finding.tier {
        humanitl_core::Tier::Checksum => v1::FindingTier::Checksum,
        humanitl_core::Tier::Regex => v1::FindingTier::Regex,
        humanitl_core::Tier::UserTerm => v1::FindingTier::UserTerm,
    };
    v1::Finding {
        kind: finding_kind_text(&finding.kind),
        location: location as i32,
        header_name,
        span_start: u64::try_from(finding.span.start).unwrap_or(u64::MAX),
        span_end: u64::try_from(finding.span.end).unwrap_or(u64::MAX),
        tier: tier as i32,
        value_hash: finding.value_hash.to_vec(),
        display_prefix: finding.display_prefix.clone(),
        resolved: false,
    }
}

/// Der Wire-Name einer Fundart, zum Beispiel `api_key.github`.
fn finding_kind_text(kind: &humanitl_core::FindingKind) -> String {
    use humanitl_core::FindingKind as Kind;
    match kind {
        Kind::ApiKey(name) | Kind::UserTerm(name) | Kind::Custom(name) => {
            format!("{}.{name}", kind.as_str())
        }
        other => other.as_str().to_owned(),
    }
}

/// Ein Zeitpunkt in der Wire-Form.
#[must_use]
pub fn timestamp(at: SystemTime) -> prost_types::Timestamp {
    at.duration_since(SystemTime::UNIX_EPOCH).map_or_else(
        |_| prost_types::Timestamp::default(),
        |since| prost_types::Timestamp {
            seconds: i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
            nanos: i32::try_from(since.subsec_nanos()).unwrap_or_default(),
        },
    )
}

/// Die Spanne zwischen zwei Zeitpunkten in der Wire-Form.
#[must_use]
pub fn duration_between(from: SystemTime, to: SystemTime) -> prost_types::Duration {
    let span = to.duration_since(from).unwrap_or(Duration::ZERO);
    prost_types::Duration {
        seconds: i64::try_from(span.as_secs()).unwrap_or(i64::MAX),
        nanos: i32::try_from(span.subsec_nanos()).unwrap_or_default(),
    }
}

/// Der registrierbare Teil eines Hostnamens, grob geschätzt.
pub(crate) fn apex_of(host: &HostName) -> String {
    match host {
        HostName::Ip(ip) => ip.to_string(),
        HostName::Dns(name) => {
            let labels: Vec<&str> = name.split('.').collect();
            if labels.len() <= 2 {
                name.clone()
            } else {
                labels[labels.len() - 2..].join(".")
            }
        }
    }
}

/// Rechnet eine Frist des Kerns in die Wanduhrzeit des Vertrags um.
///
/// Der Kern misst Fristen als [`Instant`], weil nur diese Uhr monoton ist und
/// weil die Warteschlange ihren Zeitgeber daran hängt. Der Vertrag überträgt
/// einen `Timestamp`, denn der Client rechnet keine fremde Laufzeituhr um.
/// Beide beschreiben denselben Augenblick: die verbleibende Dauer wird auf die
/// Kalenderuhr addiert. Eine Frist, die schon abgelaufen ist, ergibt „jetzt".
#[must_use]
pub fn wall_clock(deadline: Instant) -> SystemTime {
    let remaining = deadline.saturating_duration_since(Instant::now());
    SystemTime::now()
        .checked_add(remaining)
        .unwrap_or_else(SystemTime::now)
}

/// Was der Domain-Katalog über ein Ziel sagt, in der Wire-Form.
///
/// Jedes `None` des Katalogs wird zum leeren Feld beziehungsweise zur Null,
/// und beides heißt im Vertrag „unbekannt", nie „unbedenklich"
/// (`proto/humanitl/v1/humanitl.proto`, `DomainInfo`). Das Feld heißt weiter
/// `tranco_rank`, obwohl der Rust-Typ `popularity_rank` sagt; die Umbenennung
/// ist ein eigener Chore (CONVENTIONS.md 4.13, HUM-031).
#[must_use]
pub fn domain_to_proto(info: &humanitl_catalog::DomainInfo) -> v1::DomainInfo {
    v1::DomainInfo {
        apex: info.apex.clone().unwrap_or_default(),
        catalog_id: info.catalog_id.clone().unwrap_or_default(),
        tranco_rank: info.popularity_rank.unwrap_or(0),
        first_seen: info.first_seen.map(|at| prost_types::Timestamp {
            seconds: at.timestamp(),
            nanos: i32::try_from(at.timestamp_subsec_nanos()).unwrap_or(0),
        }),
        seen_count: info.seen_count,
    }
}

/// Was ein Daemon ohne Katalog über die Ziel-Domain sagen kann.
///
/// Der Rückfall für den Fake (`humanitld --fake`), der keinen Katalog lädt:
/// Die Antwort trägt nur den Apex, den `apex_of` aus dem Hostnamen ableitet.
/// Rang und Katalog-Eintrag bleiben „unbekannt" (0 und leer): eine erfundene
/// Zahl sähe in der Oberfläche wie eine gemessene aus. Der echte Daemon nimmt
/// [`domain_to_proto`] über der [`DomainTable`].
#[must_use]
pub fn domain_of(authority: &Authority, first_seen: SystemTime) -> v1::DomainInfo {
    v1::DomainInfo {
        apex: apex_of(&authority.host),
        catalog_id: String::new(),
        tranco_rank: 0,
        first_seen: Some(timestamp(first_seen)),
        seen_count: 1,
    }
}

/// Die Zeilendarstellung eines Datensatzes der Registry.
///
/// Was die Registry nicht führt, bleibt leer, statt geraten zu werden
/// (`docs/ARCHITECTURE.md` 1.3, HUM-016):
///
/// - `decision_source` kommt aus dem Datensatz, sobald ein
///   [`humanitl_core::FlowEvent::Decided`] oder ein
///   [`humanitl_core::FlowEvent::TimedOut`] durch die Registry gelaufen ist.
///   Für einen Datensatz, der schon entschieden angelegt wurde, bleibt nur,
///   was die Entscheidung selbst über ihre Herkunft sagt (`implied_source`);
///   ist auch das nichts, bleibt das Feld leer.
/// - `duration` ist gesetzt, sobald der Flow sein `Recorded` erreicht hat,
///   also die Spanne von der Ankunft bis zum Ende. Solange die Antwort noch
///   streamt, gibt es kein Ende und deshalb keine Dauer.
/// - `response_size` ist die Summe der bisher gesehenen
///   [`humanitl_core::FlowEvent::ResponseChunk`]; bis zur ersten Antwort ist
///   das null Bytes.
/// - `finding_count` zählt nur, solange der Flow in [`FlowState::Analyzed`]
///   steht; danach hält der Zustand die Funde nicht mehr.
/// - `origin_tool` ist in M1 unbekannt.
/// - `passthrough` kommt aus der Herkunft der Entscheidung: Nur die
///   Durchreichregel zum Sprachmodell entscheidet mit
///   [`humanitl_core::DecisionSource::Passthrough`]
///   (HUM-039). Solange nichts entschieden ist, ist das Feld `false` — was
///   noch offen ist, wurde nicht durchgereicht.
#[must_use]
pub fn record_to_summary(record: &FlowRecord) -> v1::FlowSummary {
    let request = &record.request;
    let (kind, block_reason, rule_id) = decision_fields(record.decision.as_ref(), None);
    v1::FlowSummary {
        flow_id: record.id.to_string(),
        session_id: record.session.to_string(),
        received_at: Some(timestamp(record.created)),
        method: method_to_proto(&request.method) as i32,
        method_raw: method_raw(&request.method),
        scheme: scheme_to_proto(request.scheme) as i32,
        authority: Some(authority_to_proto(&request.authority)),
        path: request.path_and_query.clone(),
        state: flow_state_to_proto(&record.state) as i32,
        decision: kind as i32,
        decision_source: record
            .decision_source
            .map_or_else(|| implied_source(record.decision.as_ref()), source_to_proto)
            as i32,
        block_reason: block_reason as i32,
        rule_id,
        status: u32::from(record.response_status.unwrap_or(0)),
        request_size: request.body.size,
        response_size: record.response_bytes,
        duration: record
            .finished
            .map(|at| duration_between(record.created, at)),
        finding_count: match &record.state {
            FlowState::Analyzed { findings } => u32::try_from(findings.len()).unwrap_or(u32::MAX),
            _ => 0,
        },
        edited: matches!(record.decision, Some(Decision::AllowEdited { .. })),
        passthrough: record.decision_source == Some(humanitl_core::DecisionSource::Passthrough),
        deadline: record.deadline.map(|at| timestamp(wall_clock(at))),
        origin_tool: String::new(),
        upstream_error: match &record.state {
            FlowState::Failed { error } => upstream_error_to_proto(*error) as i32,
            _ => 0,
        },
        // Die Registry im Speicher führt nur den Grund, den ihr Zustand selbst
        // nennt: `Failed { error }` für einen gescheiterten Weg nach draußen.
        // Der abgebrochene TLS-Handschlag (HUM-045) steht dort nicht, denn sein
        // Flow endet als `recorded`, und der Grund lebt allein in der Spalte
        // `flows.error`. Wer ihn braucht, liest die Aufzeichnung
        // ([`recorded_summary_to_proto`]); läuft der Daemon ohne Recorder — im
        // Fake-Betrieb etwa —, bleibt `error` bei einem TLS-Flow leer. Ihn hier
        // aus dem Zustand zu raten hieße, ihn zu erfinden.
        error: match &record.state {
            FlowState::Failed { error } => error.to_string(),
            _ => String::new(),
        },
    }
}

/// Die Herkunft, soweit die Entscheidung selbst sie nennt.
///
/// Der Rückfall für einen [`FlowRecord`], der keine Herkunft mitbekommen hat,
/// weil er schon entschieden angelegt wurde. Geraten wird nichts: nur ein
/// Ablauf und eine Regel-Blockade nennen ihre Herkunft in der Entscheidung
/// selbst.
const fn implied_source(decision: Option<&Decision>) -> v1::DecisionSource {
    match decision {
        Some(Decision::TimedOut) => v1::DecisionSource::Timeout,
        Some(Decision::Block {
            reason: BlockReason::Rule(_),
            ..
        }) => v1::DecisionSource::Rule,
        _ => v1::DecisionSource::Unspecified,
    }
}

/// Alles, was der Detail-Bereich zu einem Flow zeigt.
///
/// Die Vorschau des Bodys steht nur hier, nie in einem Ereignis
/// (`backlog/CONVENTIONS.md` 3.6). Antwort-Kopfzeilen und Funde führt die
/// Registry nicht; sie kommen mit dem Recorder (HUM-026).
#[must_use]
pub fn record_to_detail(record: &FlowRecord) -> v1::FlowDetail {
    v1::FlowDetail {
        summary: Some(record_to_summary(record)),
        request: Some(request_to_proto(&record.request)),
        edited_request: None,
        response: record.response_status.map(|status| v1::HttpResponseHead {
            status: u32::from(status),
            headers: Vec::new(),
            version: HTTP_VERSION.to_owned(),
        }),
        response_body: None,
        findings: match &record.state {
            FlowState::Analyzed { findings } => findings.iter().map(finding_to_proto).collect(),
            _ => Vec::new(),
        },
        diagnostics: Vec::new(),
        domain: Some(domain_of(&record.request.authority, record.created)),
        body_preview: body_preview(record.request.body.inline.as_deref().unwrap_or_default()),
        findings_truncated: record.findings_truncated,
    }
}

/// Alles, was der Detail-Bereich zu einem aufgezeichneten Flow zeigt.
///
/// Die Zeile, die Nachrichten und die Funde kommen aus der Aufzeichnung; alles
/// andere reicht der Aufrufer:
///
/// - `domain`, weil die Zähler der Sitzung im Prozess leben ([`DomainTable`]),
/// - `findings_truncated`, weil nur die Registry der laufenden Sitzung weiß,
///   ob der Scan die ganze Anfrage gesehen hat (die Tabelle `flows` führt die
///   Spalte nicht); für einen Flow von gestern ist die Antwort `false`, und
///   `false` heißt hier „nicht als gekürzt bekannt",
/// - `body_preview`, weil der Anfang des Bodys aus dem Blob-Speicher kommen
///   kann und dieser Weg `async` ist.
#[must_use]
pub fn recorded_detail_to_proto(
    detail: &RecordedDetail,
    domain: Option<v1::DomainInfo>,
    findings_truncated: bool,
    body_preview: String,
) -> v1::FlowDetail {
    let summary = recorded_summary_to_proto(&detail.summary);
    let request = message_of(detail, Dir::Request);
    let edited = message_of(detail, Dir::RequestEdited);
    let response = message_of(detail, Dir::Response);
    v1::FlowDetail {
        request: Some(recorded_request_to_proto(&summary, request)),
        edited_request: edited.map(|message| recorded_request_to_proto(&summary, Some(message))),
        response: response.map(|message| v1::HttpResponseHead {
            status: summary.status,
            headers: recorded_headers_to_proto(message),
            version: HTTP_VERSION.to_owned(),
        }),
        response_body: response.map(|message| body_to_proto(&message.body)),
        findings: detail
            .findings
            .iter()
            .map(recorded_finding_to_proto)
            .collect(),
        diagnostics: Vec::new(),
        domain,
        body_preview,
        findings_truncated,
        summary: Some(summary),
    }
}

/// Die aufgezeichnete Nachricht einer Richtung.
fn message_of(detail: &RecordedDetail, dir: Dir) -> Option<&MessageRecord> {
    detail.messages.iter().find(|message| message.dir == dir)
}

/// Die Kopfzeilen einer aufgezeichneten Nachricht, in Originalreihenfolge.
fn recorded_headers_to_proto(message: &MessageRecord) -> Vec<v1::Header> {
    message
        .headers
        .iter()
        .map(|(name, value)| v1::Header {
            name: name.clone(),
            value: value.clone().into_bytes(),
        })
        .collect()
}

/// Die Anfrage eines aufgezeichneten Flows.
///
/// Methode, Schema, Ziel und Pfad stehen in der Zeile des Flows, die
/// Kopfzeilen und der Body-Verweis in der Nachricht. Fehlt die Nachricht (ein
/// Flow, dessen Aufzeichnung eine Lücke hat), bleibt beides leer, statt die
/// Anfrage ganz wegzulassen.
fn recorded_request_to_proto(
    summary: &v1::FlowSummary,
    message: Option<&MessageRecord>,
) -> v1::HttpRequest {
    v1::HttpRequest {
        method: summary.method,
        method_raw: summary.method_raw.clone(),
        scheme: summary.scheme,
        authority: summary.authority.clone(),
        path_and_query: summary.path.clone(),
        headers: message.map(recorded_headers_to_proto).unwrap_or_default(),
        body: message.map(|message| body_to_proto(&message.body)),
        version: HTTP_VERSION.to_owned(),
    }
}

/// Ein aufgezeichneter Fund in der Wire-Form.
///
/// Ort und Stufe stehen in der Datenbank als Text, damit ein Archiv auch eine
/// Zeile lesen kann, die eine ältere Fassung geschrieben hat
/// (`backlog/CONVENTIONS.md` 4.14). Ein Text, den diese Fassung nicht kennt,
/// wird `UNSPECIFIED` und nicht geraten.
#[must_use]
pub fn recorded_finding_to_proto(finding: &FindingRecord) -> v1::Finding {
    let (location, header_name) = match finding.location.split_once(':') {
        Some(("header", name)) => (v1::FindingLocation::Header, name.to_owned()),
        _ => match finding.location.as_str() {
            "query" => (v1::FindingLocation::Query, String::new()),
            "body" => (v1::FindingLocation::Body, String::new()),
            _ => (v1::FindingLocation::Unspecified, String::new()),
        },
    };
    v1::Finding {
        kind: finding.kind.clone(),
        location: location as i32,
        header_name,
        span_start: finding.span_start,
        span_end: finding.span_end,
        tier: match finding.tier.as_str() {
            "checksum" => v1::FindingTier::Checksum,
            "regex" => v1::FindingTier::Regex,
            "user_term" => v1::FindingTier::UserTerm,
            _ => v1::FindingTier::Unspecified,
        } as i32,
        value_hash: finding.value_hash.to_vec(),
        display_prefix: finding.display_prefix.clone(),
        resolved: finding.resolved.is_some(),
    }
}

/// Übersetzt ein Ereignis des echten Daemons in seine Wire-Form.
///
/// Die Registry liefert dazu, was das Ereignis nicht selbst trägt: die
/// Zeilendarstellung für `Received` und die Sitzung, zu der der Flow gehört.
/// Kennt sie den Flow noch nicht — `Received` wird veröffentlicht, bevor die
/// Pipeline den Datensatz anlegt —, entsteht die Zeile aus der Anfrage im
/// Ereignis selbst.
///
/// `domains` ist die Antwort des Katalogs, die der Proxy beim Eintreffen des
/// Flows abgelegt hat ([`DomainTable`], HUM-031). Sie wird hier nur gelesen,
/// nie erhoben: Diese Funktion läuft einmal je Zuhörer, und ein Zähler, der
/// mit der Zahl der offenen Fenster stiege, wäre keine Beobachtung mehr.
/// Ohne Katalog (Fake-Modus) bleibt der Rückfall [`domain_of`].
#[must_use]
pub fn flow_event_to_proto(
    event: &FlowEvent,
    registry: &FlowRegistry,
    domains: Option<&DomainTable>,
) -> v1::FlowEvent {
    use v1::flow_event::Event;

    let flow_id = event.flow_id().map(|id| id.to_string()).unwrap_or_default();
    let wire = match event {
        FlowEvent::Received {
            flow_id: id,
            at,
            request,
        } => Event::Received(v1::flow_event::Received {
            summary: Some(registry.get(*id).map_or_else(
                || received_summary(*id, *at, request),
                |r| record_to_summary(&r),
            )),
            domain: Some(
                domains
                    .and_then(|domains| domains.get(*id))
                    .as_ref()
                    .map_or_else(|| domain_of(&request.authority, *at), domain_to_proto),
            ),
        }),
        FlowEvent::Analyzed { findings, .. } => Event::Analyzed(v1::flow_event::Analyzed {
            flow_id,
            findings: findings.iter().map(finding_to_proto).collect(),
        }),
        FlowEvent::Held {
            deadline,
            queue_bytes,
            queue_count,
            ..
        } => Event::Held(v1::flow_event::Held {
            flow_id,
            deadline: Some(timestamp(wall_clock(*deadline))),
            queue_bytes: *queue_bytes,
            queue_count: *queue_count,
        }),
        FlowEvent::Decided {
            decision, source, ..
        } => {
            let (kind, reason, rule_id) = decision_fields(Some(decision), Some(*source));
            Event::Decided(v1::flow_event::Decided {
                flow_id,
                kind: kind as i32,
                source: source_to_proto(*source) as i32,
                block_reason: reason as i32,
                rule_id,
                note: block_note(decision),
            })
        }
        FlowEvent::Forwarded { .. } => Event::Forwarded(v1::FlowRef { flow_id }),
        FlowEvent::ResponseHeaders { status, .. } => {
            Event::ResponseHeaders(v1::flow_event::ResponseHeaders {
                flow_id,
                head: Some(v1::HttpResponseHead {
                    status: u32::from(*status),
                    headers: Vec::new(),
                    version: HTTP_VERSION.to_owned(),
                }),
                streaming: false,
            })
        }
        FlowEvent::ResponseChunk { len, .. } => {
            Event::ResponseChunk(v1::flow_event::ResponseChunk {
                flow_id,
                bytes_so_far: *len,
            })
        }
        FlowEvent::Failed { error, .. } => Event::Failed(v1::flow_event::Failed {
            flow_id,
            error: upstream_error_to_proto(*error) as i32,
            resolved_ip: resolved_ip(*error),
        }),
        FlowEvent::TimedOut { .. } => Event::TimedOut(v1::FlowRef { flow_id }),
        FlowEvent::Recorded { .. } => Event::Recorded(v1::FlowRef { flow_id }),
        FlowEvent::Lagged { n } => Event::Lagged(v1::flow_event::Lagged { dropped: *n }),
        // Ein Befund mit Flow gehört an seinen Flow. Der Strom ist
        // sitzungsweit; ohne die Kennung könnte kein Client die Meldung der
        // Anfrage zuordnen, vor der sie warnt, und `LLM_005` öffnete keine
        // Flow-Details (HUM-039). Der alte Arm bleibt für alles, was zu keinem
        // Flow gehört — ein `ClientHello` ohne SNI etwa, aus dem noch gar kein
        // Flow geworden ist (`TLS_003`).
        FlowEvent::Diagnostic {
            flow_id: Some(_),
            diagnostic,
            ..
        } => Event::FlowDiagnostic(v1::flow_event::FlowDiagnostic {
            flow_id,
            diagnostic: Some(diagnostic_to_proto(diagnostic)),
        }),
        FlowEvent::Diagnostic { diagnostic, .. } => {
            Event::Diagnostic(diagnostic_to_proto(diagnostic))
        }
        // Die Bitte des Agenten gehört zu keinem Flow: Sie hat ihre eigene
        // Kennung, und `flow_id` bleibt leer (HUM-073).
        FlowEvent::AgentAsk { .. } => Event::AgentAsk(agent_ask_to_proto(event)),
    };
    v1::FlowEvent {
        at: Some(timestamp(event.at().unwrap_or_else(SystemTime::now))),
        event: Some(wire),
    }
}

/// Die aufgelöste Adresse eines gescheiterten Flows, als Text.
///
/// Nur [`UpstreamError::PrivateAddress`] trägt eine; jeder andere Fehler lässt
/// das Feld leer, statt eine Adresse zu erfinden, die niemand gesehen hat.
fn resolved_ip(error: UpstreamError) -> String {
    match error {
        UpstreamError::PrivateAddress(ip) => ip.to_string(),
        UpstreamError::Dns
        | UpstreamError::Connect
        | UpstreamError::Tls
        | UpstreamError::Timeout => String::new(),
    }
}

/// Die Wire-Form einer Bitte des Agenten (HUM-073).
///
/// Der Text ist im Proxy schon gesäubert (`sanitize_note`); hier wird nichts
/// mehr daran geändert, damit es nur eine Stelle gibt, die das tut. Ein
/// fehlender Vorschlag reist als leere Zeichenkette: `proto3` kennt kein
/// `optional string` ohne zusätzliche Kennzeichnung, und leer heißt hier „es
/// stand keine URL im Text".
fn agent_ask_to_proto(event: &FlowEvent) -> v1::flow_event::AgentAsk {
    let FlowEvent::AgentAsk {
        ask_id,
        text,
        suggested_host,
        suggested_path,
        ..
    } = event
    else {
        // Der Aufrufer prüft die Variante; ein anderes Ereignis kommt hier
        // nicht an, und eine leere Bitte ist die harmlose Antwort darauf.
        return v1::flow_event::AgentAsk::default();
    };
    v1::flow_event::AgentAsk {
        ask_id: ask_id.to_string(),
        text: text.clone(),
        suggested_host: suggested_host.clone().unwrap_or_default(),
        suggested_path: suggested_path.clone().unwrap_or_default(),
    }
}

/// Die Zeile eines Flows, den die Registry noch nicht kennt.
///
/// `Received` entsteht im Handler, bevor die Pipeline den Datensatz anlegt.
/// Was nur die Registry wüsste — die Sitzung —, bleibt hier leer; der Zustand
/// ist der eines gerade angekommenen Flows.
fn received_summary(id: FlowId, at: SystemTime, request: &HttpRequest) -> v1::FlowSummary {
    v1::FlowSummary {
        flow_id: id.to_string(),
        session_id: String::new(),
        received_at: Some(timestamp(at)),
        method: method_to_proto(&request.method) as i32,
        method_raw: method_raw(&request.method),
        scheme: scheme_to_proto(request.scheme) as i32,
        authority: Some(authority_to_proto(&request.authority)),
        path: request.path_and_query.clone(),
        state: v1::FlowState::Received as i32,
        request_size: request.body.size,
        ..v1::FlowSummary::default()
    }
}

// ---------- Auswahl auf der Wire-Form ----------
//
// Filter, Anker und Seiten arbeiten auf [`v1::FlowSummary`], nicht auf den
// Kern-Typen: der Fake hat keine Registry, der echte Dienst keine Sitzungsdatei,
// aber beide beantworten `ListFlows` und `Subscribe(since_flow_id)`. Die
// Auswahl steht deshalb hier, damit sie für beide dieselbe ist und die
// Oberfläche keinen Unterschied sieht.

/// Ob ein Flow hinter einem Anker liegt.
///
/// Ohne Anker passt jeder Flow. [`FlowId`] ist ein UUID der Fassung 7; die Ordnung der Ids
/// ist die Ordnung der Ankunft.
#[must_use]
pub fn after(summary: &v1::FlowSummary, anchor: Option<FlowId>) -> bool {
    match anchor {
        None => true,
        Some(anchor) => FlowId::parse(&summary.flow_id).is_ok_and(|id| id > anchor),
    }
}

/// Gegenstück zu [`after`] für absteigende Seiten: nur Flows vor dem Anker.
#[must_use]
pub fn before(summary: &v1::FlowSummary, anchor: Option<FlowId>) -> bool {
    match anchor {
        None => true,
        Some(anchor) => FlowId::parse(&summary.flow_id).is_ok_and(|id| id < anchor),
    }
}

/// Ob ein Flow zum Filtertext passt.
///
/// Unterstützt `host:<text>`, `state:<name>` und `session:<id>`; alles andere ist eine
/// Teilzeichenkette über Host und Pfad. Die vollständige Filtersprache des
/// History-Screens baut HUM-030.
#[must_use]
pub fn matches_filter(summary: &v1::FlowSummary, filter: &str) -> bool {
    filter.split_whitespace().all(|token| {
        let host = summary
            .authority
            .as_ref()
            .map(|authority| authority.host.clone())
            .unwrap_or_default();
        match token.split_once(':') {
            Some(("host", value)) => host.contains(value),
            Some(("state", value)) => state_name(summary.state).eq_ignore_ascii_case(value),
            Some(("session", value)) => summary.session_id.contains(value),
            _ => host.contains(token) || summary.path.contains(token),
        }
    })
}

/// Der Kurzname eines Zustands, wie ihn der Filter erwartet.
#[must_use]
pub fn state_name(state: i32) -> &'static str {
    v1::FlowState::try_from(state)
        .unwrap_or(v1::FlowState::Unspecified)
        .as_str_name()
        .trim_start_matches("FLOW_STATE_")
}

/// Warum eine Regel von der Leitung nicht lesbar war.
///
/// Der Daemon lehnt sie mit dem Befund ab, statt sie zu ergänzen: eine Regel,
/// die anders gilt, als der Mensch sie geschrieben hat, ist schlimmer als
/// keine (ADR-007).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleError {
    /// `RULE_ACTION_UNSPECIFIED`; eine Regel ohne Aktion gibt es nicht.
    #[error("the rule has no action")]
    Action,
    /// Die Bedingung fehlt ganz.
    #[error("the rule has no matcher")]
    Matcher,
    /// Das Host-Muster ließ sich nicht lesen.
    #[error("{0}")]
    Host(#[from] humanitl_core::rule::HostPatternError),
    /// Eine Methode ist unbekannt oder `METHOD_UNSPECIFIED`.
    #[error("{0:?} is not one of the known http methods")]
    Method(String),
    /// Der Port liegt außerhalb von `1..=65535`.
    #[error("{0} is not a port; the range is 1..=65535")]
    Port(u32),
    /// Eine Id (`rule_id`, `created_from_flow_id`) war kein UUID-Text.
    #[error("{field} is not a valid id: {value:?}")]
    Id {
        /// Welches Feld.
        field: &'static str,
        /// Der abgelehnte Text.
        value: String,
    },
    /// Der Zeitpunkt in `expires.at` liegt außerhalb dessen, was eine Uhr kennt.
    #[error("the expiry timestamp is out of range")]
    Expiry,
}

/// Liest eine Regel aus ihrer Wire-Form.
///
/// `session` ist die laufende Sitzung; `expires: session` gehört immer ihr.
/// Was der Daemon selbst vergibt, wird nicht von der Leitung übernommen: eine
/// leere `rule_id` wird erzeugt, `position`, `hit_count` und `created_at` sind
/// Ausgabefelder, und `bundled` ist immer `false` — eine mitgelieferte Regel
/// entsteht nur beim Laden von `rules/default.yaml`, sonst könnte sich ein
/// Client eine unlöschbare Regel anlegen.
///
/// Die Notiz läuft durch [`humanitl_core::block::sanitize_note`], weil sie in
/// `rules.yaml` und in der Oberfläche landet; unsichtbare Zeichen haben in
/// beidem nichts zu suchen (HUM-072).
///
/// # Errors
///
/// [`RuleError`], wenn ein Feld nicht lesbar ist. Das Pfadmuster wird hier
/// nicht übersetzt; das tut die Engine beim Speichern, mit `RULES_005`.
pub fn rule_from_proto(proto: &v1::Rule, session: SessionId) -> Result<Rule, RuleError> {
    use humanitl_core::rule::{Action, HostPattern, Matcher, PathPattern};

    let action = match v1::RuleAction::try_from(proto.action).unwrap_or(v1::RuleAction::Unspecified)
    {
        v1::RuleAction::Allow => Action::Allow,
        v1::RuleAction::Block => Action::Block,
        v1::RuleAction::Ask => Action::Ask,
        v1::RuleAction::Redact => Action::Redact,
        v1::RuleAction::Unspecified => return Err(RuleError::Action),
    };
    let wire = proto.matcher.as_ref().ok_or(RuleError::Matcher)?;

    let mut matcher = Matcher::host(HostPattern::parse(&wire.host)?);
    if !wire.methods.is_empty() {
        let mut methods = Vec::with_capacity(wire.methods.len());
        for raw in &wire.methods {
            methods.push(rule_method_from_proto(*raw)?);
        }
        matcher.methods = Some(methods);
    }
    if !wire.path.is_empty() {
        matcher.path = Some(PathPattern::parse(&wire.path));
    }
    // Ein unbrauchbarer Präfix wird nicht stillschweigend weggelassen: Er
    // stünde sonst in der Regel, die der Mensch abgeschickt hat, und nicht in
    // der, die gilt. Die Engine sortiert ihn beim Übersetzen aus und lässt die
    // Regel dann nichts treffen (`humanitl_rules::eval`); dieselbe sichere
    // Seite wie bei einem Pfadmuster, das sich nicht übersetzen lässt.
    matcher.path_prefixes.clone_from(&wire.path_prefixes);
    matcher.scheme = match v1::Scheme::try_from(wire.scheme).unwrap_or(v1::Scheme::Unspecified) {
        v1::Scheme::Unspecified => None,
        v1::Scheme::Http => Some(humanitl_core::http::Scheme::Http),
        v1::Scheme::Https => Some(humanitl_core::http::Scheme::Https),
        v1::Scheme::Ws => Some(humanitl_core::http::Scheme::Ws),
        v1::Scheme::Wss => Some(humanitl_core::http::Scheme::Wss),
    };
    matcher.port = match wire.port {
        0 => None,
        port => Some(u16::try_from(port).map_err(|_| RuleError::Port(port))?),
    };
    matcher.upgrade = match v1::Upgrade::try_from(wire.upgrade).unwrap_or(v1::Upgrade::Unspecified)
    {
        v1::Upgrade::Websocket => Some(humanitl_core::http::Upgrade::WebSocket),
        v1::Upgrade::Unspecified | v1::Upgrade::None => None,
    };

    let id = if proto.rule_id.is_empty() {
        RuleId::new()
    } else {
        RuleId::parse(&proto.rule_id).map_err(|_| RuleError::Id {
            field: "rule_id",
            value: proto.rule_id.clone(),
        })?
    };
    let mut rule = Rule::new(id, action, matcher)
        .with_expiry(expiry_from_proto(proto.expires.as_ref(), session)?)
        .with_stream(proto.stream)
        .with_allow_private(proto.allow_private)
        // Eine Regel, die über den Draht kommt, darf abgeschaltet ankommen;
        // wirksam wird das nur für mitgelieferte Regeln, und die kommen nie
        // über den Draht (`RulesStore::set_bundled_disabled`).
        .disabled(proto.disabled)
        // `passthrough_llm` wird von der Leitung nie übernommen, aus demselben
        // Grund wie `bundled`: Ein Client, der sich eine Durchreichregel
        // anlegen könnte, könnte damit Verkehr an der Warteschlange und an der
        // voreingestellten Ansicht vorbeiführen — die eine erklärte Ausnahme
        // ist keine, die ein Aufruf sich selbst ausstellt (HUM-039).
        .passthrough_llm(false);
    if !proto.created_from_flow_id.is_empty() {
        rule.created_from =
            Some(
                FlowId::parse(&proto.created_from_flow_id).map_err(|_| RuleError::Id {
                    field: "created_from_flow_id",
                    value: proto.created_from_flow_id.clone(),
                })?,
            );
    }
    let note = humanitl_core::block::sanitize_note(&proto.note);
    rule.note = (!note.is_empty()).then_some(note);
    Ok(rule)
}

/// Liest die Methode einer Regel aus ihrer Wire-Form.
///
/// Anders als [`method_from_proto`] gibt es hier keinen Rohwert: eine Regel
/// wird über bekannten Methoden geschrieben, und `METHOD_OTHER` wäre eine
/// Regel über allem, was der Daemon nicht versteht (`humanitl_rules`
/// `is_known_method`).
fn rule_method_from_proto(raw: i32) -> Result<Method, RuleError> {
    let wire = v1::Method::try_from(raw).unwrap_or(v1::Method::Unspecified);
    let name = wire.as_str_name().trim_start_matches("METHOD_");
    match wire {
        v1::Method::Unspecified | v1::Method::Other => Err(RuleError::Method(name.to_owned())),
        _ => Method::from_bytes(name.as_bytes()).map_err(|_| RuleError::Method(name.to_owned())),
    }
}

/// Liest die Gültigkeit aus ihrer Wire-Form.
///
/// Ohne Angabe gilt `never`: eine Regel ohne Ablauf ist der Normalfall, und
/// ein Client, der das Feld nicht kennt, soll keine Regel bekommen, die nach
/// dem Neustart weg ist.
fn expiry_from_proto(
    expires: Option<&v1::RuleExpiry>,
    session: SessionId,
) -> Result<Expiry, RuleError> {
    let Some(expiry) = expires.and_then(|wrapper| wrapper.expiry.as_ref()) else {
        return Ok(Expiry::Never);
    };
    match expiry {
        v1::rule_expiry::Expiry::Never(()) => Ok(Expiry::Never),
        v1::rule_expiry::Expiry::Session(()) => Ok(Expiry::Session(session)),
        v1::rule_expiry::Expiry::At(at) => {
            chrono::DateTime::from_timestamp(at.seconds, u32::try_from(at.nanos).unwrap_or(0))
                .map(Expiry::At)
                .ok_or(RuleError::Expiry)
        }
    }
}

/// Die Wire-Form einer Regel samt Platz und Herkunft aus dem Speicher.
///
/// Die Herkunft steht nicht als eigenes Feld im Vertrag: `bundled` und
/// `expires` sagen sie schon (`session` ⇒ Sitzungsregel, `bundled` ⇒
/// mitgeliefert, sonst dauerhaft). Doppelt geführt liefen beide auseinander.
#[must_use]
pub fn stored_rule_to_proto(stored: &StoredRule) -> v1::Rule {
    v1::Rule {
        position: stored.position,
        ..rule_to_proto(&stored.rule)
    }
}

/// Die Zeilendarstellung einer aufgezeichneten Anfrage.
///
/// Was die Aufzeichnung nicht führt, bleibt leer statt geraten: `deadline`
/// gehört zur laufenden Warteschlange, `origin_tool` kommt mit HUM-030,
/// `decision_source` steht nicht in der Tabelle `flows` (nur `rule_id` verrät
/// eine Regel, und `timed_out` seinen Ablauf).
#[must_use]
pub fn recorded_summary_to_proto(row: &RecordedSummary) -> v1::FlowSummary {
    let method = Method::from_bytes(row.method.as_bytes()).unwrap_or(Method::GET);
    let scheme = humanitl_core::http::Scheme::parse(&row.scheme)
        .unwrap_or(humanitl_core::http::Scheme::Https);
    let decision = row.decision.as_deref().unwrap_or_default();
    v1::FlowSummary {
        flow_id: row.id.to_string(),
        session_id: row.session.to_string(),
        received_at: Some(timestamp_from_millis(row.ts)),
        method: method_to_proto(&method) as i32,
        method_raw: method_raw(&method),
        scheme: scheme_to_proto(scheme) as i32,
        authority: Some(v1::Authority {
            host: row.host.clone(),
            port: u32::from(row.port),
            is_ip_literal: row.host.parse::<std::net::IpAddr>().is_ok(),
            display_host: row.host_display.clone(),
        }),
        path: row.path.clone(),
        state: state_from_name(&row.state) as i32,
        decision: decision_kind_from_name(decision) as i32,
        decision_source: match decision {
            "timed_out" => v1::DecisionSource::Timeout as i32,
            _ if row.rule_id.is_some() => v1::DecisionSource::Rule as i32,
            _ => v1::DecisionSource::Unspecified as i32,
        },
        block_reason: block_reason_from_name(row.block_reason.as_deref().unwrap_or_default())
            as i32,
        rule_id: row.rule_id.clone().unwrap_or_default(),
        status: u32::from(row.status.unwrap_or(0)),
        request_size: row.request_size,
        response_size: row.response_size.unwrap_or(0),
        duration: row.duration_ms.map(|ms| prost_types::Duration {
            seconds: ms / 1_000,
            nanos: i32::try_from((ms % 1_000) * 1_000_000).unwrap_or(0),
        }),
        finding_count: row.findings_count,
        edited: row.edited,
        passthrough: row.passthrough,
        deadline: None,
        origin_tool: String::new(),
        upstream_error: 0,
        // Der Grund steht in der Spalte, nicht im Zustand: Ein abgebrochener
        // TLS-Handschlag endet als `recorded`, nicht als `failed` (HUM-045).
        error: row.error.clone().unwrap_or_default(),
    }
}

/// Ein Zeitpunkt in Unix-Millisekunden als Wire-Zeitstempel.
fn timestamp_from_millis(ms: i64) -> prost_types::Timestamp {
    prost_types::Timestamp {
        seconds: ms.div_euclid(1_000),
        nanos: i32::try_from(ms.rem_euclid(1_000) * 1_000_000).unwrap_or(0),
    }
}

/// Der Zustand hinter seinem Kurznamen ([`FlowState::name`]).
fn state_from_name(name: &str) -> v1::FlowState {
    match name {
        "received" => v1::FlowState::Received,
        "analyzed" => v1::FlowState::Analyzed,
        "held" => v1::FlowState::Held,
        "decided" => v1::FlowState::Decided,
        "forwarded" => v1::FlowState::Forwarded,
        "responded" => v1::FlowState::Responded,
        "failed" => v1::FlowState::Failed,
        "recorded" => v1::FlowState::Recorded,
        _ => v1::FlowState::Unspecified,
    }
}

/// Die Entscheidung hinter ihrem Kurznamen ([`Decision::as_str`]).
fn decision_kind_from_name(name: &str) -> v1::DecisionKind {
    match name {
        "allow" => v1::DecisionKind::Allow,
        "allow_edited" => v1::DecisionKind::AllowEdited,
        "block" => v1::DecisionKind::Block,
        "timed_out" => v1::DecisionKind::TimedOut,
        _ => v1::DecisionKind::Unspecified,
    }
}

/// Der Block-Grund hinter seinem Kurznamen ([`BlockReason::as_str`]).
fn block_reason_from_name(name: &str) -> v1::BlockReason {
    match name {
        "user" => v1::BlockReason::User,
        "rule" => v1::BlockReason::Rule,
        "timeout" => v1::BlockReason::Timeout,
        "body_cap" => v1::BlockReason::BodyCap,
        "authority_mismatch" => v1::BlockReason::AuthorityMismatch,
        "no_route" => v1::BlockReason::NoRoute,
        "hold_memory" => v1::BlockReason::HoldMemory,
        "hold_max_flows" => v1::BlockReason::HoldMaxFlows,
        "client_timeout" => v1::BlockReason::ClientTimeout,
        "private_address" => v1::BlockReason::PrivateAddress,
        "secret" => v1::BlockReason::Secret,
        _ => v1::BlockReason::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, Instant, SystemTime};

    use humanitl_config::Limits;
    use humanitl_core::{
        Authority, BodyRef, Decision, DecisionSource, Diagnostic, Flow, FlowEvent, FlowId,
        FlowState, HostName, HttpRequest, Method, Scheme, SessionId, Severity, TransitionInput,
        UpstreamError,
    };
    use humanitl_proxy::ConnMeta;
    use humanitl_proxy::registry::{FlowRecord, FlowRegistry};

    use super::{
        CheckResult, IsolationCheck, check_result_to_proto, diagnostic_to_proto,
        flow_event_to_proto, matches_filter, record_to_detail, record_to_summary, rule_from_proto,
        rule_to_proto, wall_clock,
    };
    use crate::v1;

    /// Was von der Leitung kommt, darf sich keine Durchreiche ausstellen.
    ///
    /// `passthrough_llm` bedeutet: nicht gehalten, in der voreingestellten
    /// Ansicht nicht sichtbar. Ein Client, der sich das selbst setzen könnte,
    /// könnte Verkehr an der Warteschlange vorbeiführen (HUM-039). Dieselbe
    /// Zusage wie bei `bundled`.
    #[test]
    fn a_rule_from_the_wire_can_never_be_a_passthrough() {
        let wire = v1::Rule {
            action: v1::RuleAction::Allow as i32,
            matcher: Some(v1::RuleMatcher {
                host: "ip:192.168.1.50".to_owned(),
                path_prefixes: vec!["/v1/".to_owned()],
                port: 11434,
                ..v1::RuleMatcher::default()
            }),
            passthrough_llm: true,
            bundled: true,
            allow_private: true,
            ..v1::Rule::default()
        };
        let rule = rule_from_proto(&wire, SessionId::new()).expect("the rule is readable");
        assert!(!rule.passthrough_llm, "the exception is not self-service");
        assert!(!rule.bundled, "and neither is being bundled");
        assert_eq!(
            rule.matcher.path_prefixes,
            vec!["/v1/".to_owned()],
            "the prefixes do come from the wire; they only ever narrow"
        );
    }

    /// In die andere Richtung steht das Merkmal an der Regel, damit die
    /// Oberfläche sie als das zeigen kann, was sie ist.
    #[test]
    fn a_passthrough_rule_says_so_on_the_wire() {
        let rule = humanitl_core::Rule::new(
            humanitl_core::RuleId::new(),
            humanitl_core::rule::Action::Allow,
            humanitl_core::Matcher::host(
                humanitl_core::HostPattern::parse("ip:192.168.1.50").unwrap(),
            )
            .with_path_prefixes(vec!["/v1/".to_owned(), "/api/chat".to_owned()]),
        )
        .passthrough_llm(true)
        .with_allow_private(true);

        let wire = rule_to_proto(&rule);
        assert!(wire.passthrough_llm);
        assert!(wire.allow_private);
        assert_eq!(
            wire.matcher.expect("a matcher").path_prefixes,
            vec!["/v1/".to_owned(), "/api/chat".to_owned()]
        );
    }

    fn flow(session: SessionId, host: &str) -> Flow {
        let request = HttpRequest::new(
            Method::POST,
            Scheme::Https,
            Authority::with_scheme(HostName::Dns(host.to_owned()), Scheme::Https),
            "/v1/chat?stream=true",
        )
        .with_body(BodyRef::detached([7; 32], 42));
        Flow::new(FlowId::new(), session, SystemTime::now(), request)
    }

    #[test]
    fn a_deadline_becomes_a_wall_clock_time_in_the_future() {
        let now = SystemTime::now();
        let at = wall_clock(Instant::now() + Duration::from_secs(30));
        let ahead = at.duration_since(now).unwrap();
        assert!(
            ahead > Duration::from_secs(29) && ahead < Duration::from_secs(31),
            "{ahead:?}"
        );

        // Eine abgelaufene Frist liegt nicht in der Vergangenheit, sondern jetzt.
        let elapsed = Instant::now()
            .checked_sub(Duration::from_secs(30))
            .unwrap_or_else(Instant::now);
        let past = wall_clock(elapsed);
        assert!(past.duration_since(now).unwrap() < Duration::from_secs(1));
    }

    /// Der Filter kennt `session:`, `host:` und `state:`; ein Text ohne
    /// Praefix sucht in Host und Pfad.
    #[test]
    fn the_filter_knows_session_host_and_state() {
        let session = SessionId::new();
        let flow = flow(session, "api.example.com");
        let row = record_to_summary(&FlowRecord::new(&flow, &ConnMeta::plain(session)));
        let id = session.to_string();

        assert!(matches_filter(&row, &format!("session:{id}")));
        assert!(!matches_filter(&row, "session:nobody"));
        assert!(matches_filter(&row, "host:example state:received"));
        assert!(!matches_filter(&row, "host:example state:held"));
        assert!(matches_filter(&row, "chat"));
        assert!(!matches_filter(&row, "nothing-like-this"));
    }

    #[test]
    fn a_record_becomes_a_row_without_inventing_anything() {
        let session = SessionId::new();
        let flow = flow(session, "api.example.com");
        let record = FlowRecord::new(&flow, &ConnMeta::plain(session));

        let row = record_to_summary(&record);
        assert_eq!(row.flow_id, flow.id.to_string());
        assert_eq!(row.session_id, session.to_string());
        assert_eq!(row.method, v1::Method::Post as i32);
        assert_eq!(row.scheme, v1::Scheme::Https as i32);
        assert_eq!(row.path, "/v1/chat?stream=true");
        assert_eq!(row.state, v1::FlowState::Received as i32);
        assert_eq!(row.request_size, 42);
        assert_eq!(row.authority.unwrap().host, "api.example.com");
        assert_eq!(row.decision, v1::DecisionKind::Unspecified as i32);
        assert_eq!(row.decision_source, v1::DecisionSource::Unspecified as i32);
        assert!(row.deadline.is_none(), "nothing is held here");
        assert!(row.duration.is_none(), "the flow has not ended yet");
        assert_eq!(row.response_size, 0, "no chunk has passed yet");
        assert_eq!(row.status, 0);
        assert!(row.origin_tool.is_empty());
    }

    #[test]
    fn a_finished_flow_carries_its_source_its_size_and_its_duration() {
        let session = SessionId::new();
        let mut flow = flow(session, "api.example.com");
        let registry = FlowRegistry::new(&Limits::default());
        registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));

        for input in [
            TransitionInput::Analyze { findings: vec![] },
            TransitionInput::Hold {
                deadline: Instant::now() + Duration::from_secs(30),
                queue_bytes: 42,
                queue_count: 1,
            },
            TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::User,
            },
            TransitionInput::Forward,
            TransitionInput::Respond { status: 200 },
        ] {
            let event = flow.apply(input, SystemTime::now()).unwrap();
            registry.record(&event);
        }
        registry.record(&FlowEvent::ResponseChunk {
            flow_id: flow.id,
            at: SystemTime::now(),
            len: 1024,
        });
        let end = flow
            .received_at
            .checked_add(Duration::from_millis(250))
            .unwrap();
        let recorded = flow.apply(TransitionInput::Record, end).unwrap();
        registry.record(&recorded);

        let row = record_to_summary(&registry.get(flow.id).unwrap());
        assert_eq!(row.decision_source, v1::DecisionSource::User as i32);
        assert_eq!(row.response_size, 1024);
        let duration = row.duration.expect("the flow has ended");
        assert_eq!(duration.seconds, 0);
        assert_eq!(duration.nanos, 250_000_000);
    }

    #[test]
    fn a_detail_shows_the_request_and_the_apex_but_no_body() {
        let session = SessionId::new();
        let flow = flow(session, "cdn.api.example.com");
        let record = FlowRecord::new(&flow, &ConnMeta::plain(session));

        let detail = record_to_detail(&record);
        assert_eq!(detail.domain.unwrap().apex, "example.com");
        let request = detail.request.unwrap();
        assert_eq!(request.body.unwrap().size, 42);
        assert!(
            detail.body_preview.is_empty(),
            "a detached body has nothing to preview"
        );
        assert!(detail.response.is_none());
    }

    #[test]
    fn the_state_of_a_record_reaches_the_row() {
        let session = SessionId::new();
        let mut flow = flow(session, "api.example.com");
        let registry = FlowRegistry::new(&Limits::default());
        registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));

        for input in [
            TransitionInput::Analyze { findings: vec![] },
            TransitionInput::Hold {
                deadline: Instant::now() + Duration::from_secs(30),
                queue_bytes: 42,
                queue_count: 1,
            },
            TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::User,
            },
            TransitionInput::Forward,
        ] {
            let event = flow.apply(input, SystemTime::now()).unwrap();
            registry.record(&event);
        }

        let row = record_to_summary(&registry.get(flow.id).unwrap());
        assert_eq!(row.state, v1::FlowState::Forwarded as i32);
        assert_eq!(row.decision, v1::DecisionKind::Allow as i32);
        assert!(!row.edited);
    }

    #[test]
    fn received_translates_even_before_the_registry_knows_the_flow() {
        let registry = FlowRegistry::new(&Limits::default());
        let flow = flow(SessionId::new(), "api.example.com");

        let event = flow_event_to_proto(&flow.received_event(), &registry, None);
        let Some(v1::flow_event::Event::Received(received)) = event.event else {
            panic!("received");
        };
        let summary = received.summary.unwrap();
        assert_eq!(summary.flow_id, flow.id.to_string());
        assert_eq!(summary.state, v1::FlowState::Received as i32);
        assert!(
            summary.session_id.is_empty(),
            "the session is only known to the registry"
        );
        assert_eq!(received.domain.unwrap().apex, "example.com");
    }

    #[test]
    fn a_held_event_carries_the_deadline_and_the_queue_counters() {
        let registry = FlowRegistry::new(&Limits::default());
        let session = SessionId::new();
        let mut flow = flow(session, "api.example.com");
        flow.apply(
            TransitionInput::Analyze { findings: vec![] },
            SystemTime::now(),
        )
        .unwrap();
        let event = flow
            .apply(
                TransitionInput::Hold {
                    deadline: Instant::now() + Duration::from_secs(60),
                    queue_bytes: 42,
                    queue_count: 1,
                },
                SystemTime::now(),
            )
            .unwrap();

        let wire = flow_event_to_proto(&event, &registry, None);
        let Some(v1::flow_event::Event::Held(held)) = wire.event else {
            panic!("held");
        };
        assert_eq!(held.queue_bytes, 42);
        assert_eq!(held.queue_count, 1);
        let deadline = held.deadline.unwrap();
        assert!(
            deadline.seconds > 0,
            "an absolute timestamp, not a duration"
        );
    }

    #[test]
    fn a_failed_event_names_the_private_address_it_refused() {
        use std::net::{IpAddr, Ipv4Addr};

        let registry = FlowRegistry::new(&Limits::default());
        let event = humanitl_core::FlowEvent::Failed {
            flow_id: FlowId::new(),
            at: SystemTime::now(),
            error: UpstreamError::PrivateAddress(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))),
        };

        let wire = flow_event_to_proto(&event, &registry, None);
        let Some(v1::flow_event::Event::Failed(failed)) = wire.event else {
            panic!("failed");
        };
        assert_eq!(failed.error, v1::UpstreamError::PrivateAddress as i32);
        assert_eq!(failed.resolved_ip, "10.0.0.5");
    }

    #[test]
    fn a_lagged_event_keeps_its_count() {
        let registry = FlowRegistry::new(&Limits::default());
        let wire =
            flow_event_to_proto(&humanitl_core::FlowEvent::Lagged { n: 12 }, &registry, None);
        let Some(v1::flow_event::Event::Lagged(lagged)) = wire.event else {
            panic!("lagged");
        };
        assert_eq!(lagged.dropped, 12);
    }

    #[test]
    fn a_failed_flow_shows_its_error_in_the_row() {
        let session = SessionId::new();
        let mut flow = flow(session, "api.example.com");
        let registry = FlowRegistry::new(&Limits::default());
        registry.insert(FlowRecord::new(&flow, &ConnMeta::plain(session)));
        for input in [
            TransitionInput::Analyze { findings: vec![] },
            TransitionInput::Hold {
                deadline: Instant::now() + Duration::from_secs(30),
                queue_bytes: 42,
                queue_count: 1,
            },
            TransitionInput::Decide {
                decision: Decision::Allow,
                source: DecisionSource::User,
            },
            TransitionInput::Forward,
            TransitionInput::Fail {
                error: UpstreamError::Dns,
            },
        ] {
            let event = flow.apply(input, SystemTime::now()).unwrap();
            registry.record(&event);
        }

        let record = registry.get(flow.id).unwrap();
        assert!(matches!(record.state, FlowState::Failed { .. }));
        let row = record_to_summary(&record);
        assert_eq!(row.state, v1::FlowState::Failed as i32);
        assert_eq!(row.upstream_error, v1::UpstreamError::Dns as i32);
    }

    /// Ein Socket-Name aus `/work` gehört dem Agenten, und er steht als Text
    /// neben dem Punkt, an dem ein Mensch entscheidet, ob er der Sandbox
    /// glaubt. Eine Rechts-nach-links-Marke stellte diese Zeile um, ohne dass
    /// im Text etwas anderes stünde.
    #[test]
    fn the_evidence_loses_bidi_marks_and_zero_width_characters() {
        let hostile = "single_socket FAIL: sockets=/work/\u{202e}kcos.evil\u{200b};\
                       unexpected=/work/\u{feff}x.sock";
        let proto = check_result_to_proto(&CheckResult {
            check: IsolationCheck::SingleSocket,
            passed: false,
            evidence: hostile.to_owned(),
            diagnostic: None,
        });
        for invisible in ['\u{202e}', '\u{200b}', '\u{feff}'] {
            assert!(
                !proto.evidence.contains(invisible),
                "{invisible:?} survived: {:?}",
                proto.evidence
            );
        }
        // Der lesbare Teil bleibt stehen; gesäubert wird, nicht verworfen.
        assert!(proto.evidence.contains("/work/kcos.evil"), "{proto:?}");
        assert_eq!(proto.check, v1::IsolationCheck::SingleSocket as i32);
        assert!(!proto.passed);
    }

    /// Vier Kibibyte Füllung in einem Dateinamen sind kein Beleg, sondern ein
    /// Panel, das nichts anderes mehr zeigt.
    #[test]
    fn the_evidence_is_capped_in_length() {
        let long = format!(
            "single_socket FAIL: sockets=/work/{}.sock",
            "a".repeat(4096)
        );
        let proto = check_result_to_proto(&CheckResult {
            check: IsolationCheck::SingleSocket,
            passed: false,
            evidence: long,
            diagnostic: None,
        });
        assert!(
            proto.evidence.chars().count() <= humanitl_core::block::NOTE_MAX_CHARS,
            "{} characters survived",
            proto.evidence.chars().count()
        );
    }

    /// Eine zweite `CHECK`-Zeile lässt sich durch die Evidenz nicht fälschen,
    /// und ein Steuerzeichen kommt nicht in die Oberfläche.
    #[test]
    fn the_evidence_carries_no_newline_and_no_control_character() {
        let proto = check_result_to_proto(&CheckResult {
            check: IsolationCheck::NoNetworkInterface,
            passed: true,
            evidence: "no_interfaces ok: lo\nCHECK single_socket ok: forged\u{0007}".to_owned(),
            diagnostic: None,
        });
        assert!(!proto.evidence.contains('\n'), "{proto:?}");
        assert!(!proto.evidence.contains('\u{0007}'), "{proto:?}");
    }

    /// Der Befund trägt denselben Text wie die Evidenz und geht durch
    /// dieselbe Säuberung. `bwrap.rs` baut `why` aus der rohen Zeile des
    /// Shims; ohne diesen Schritt erreichte eine Bidi-Marke die Karte.
    #[test]
    fn the_why_of_a_finding_is_sanitised_too() {
        let hostile = format!(
            "single_socket: unexpected=/work/\u{202e}kcos.evil\u{200b} {}",
            "a".repeat(4096)
        );
        let proto = diagnostic_to_proto(
            &Diagnostic::builder(
                humanitl_core::diagnostics::codes::SANDBOX_015,
                Severity::Blocking,
            )
            .why(hostile)
            .build(),
        );
        for invisible in ['\u{202e}', '\u{200b}'] {
            assert!(!proto.why.contains(invisible), "{invisible:?} survived");
        }
        assert!(proto.why.contains("/work/kcos.evil"), "{:?}", proto.why);
        assert!(
            proto.why.chars().count() <= humanitl_core::block::NOTE_MAX_CHARS,
            "{} characters survived",
            proto.why.chars().count()
        );
    }

    /// Der Befund reist als `Diagnostic`, nicht als Text.
    #[test]
    fn a_failed_check_carries_its_diagnostic() {
        let proto = check_result_to_proto(&CheckResult {
            check: IsolationCheck::SeccompActive,
            passed: false,
            evidence: "seccomp_applied FAIL: Seccomp:0".to_owned(),
            diagnostic: Some(
                Diagnostic::builder(
                    humanitl_core::diagnostics::codes::SANDBOX_016,
                    Severity::Blocking,
                )
                .why("seccomp is not active in the sandbox".to_owned())
                .build(),
            ),
        });
        let diagnostic = proto.diagnostic.expect("a failed check carries a finding");
        assert_eq!(diagnostic.code, "SANDBOX_016");
        assert_eq!(diagnostic.severity, v1::Severity::Blocking as i32);
        assert!(!diagnostic.why.is_empty(), "a finding without a why");
    }
}
