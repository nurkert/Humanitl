//! Der Meta-Endpunkt `humanitl.internal` (HUM-073, ADR-014).
//!
//! Der Agent in der Sandbox hat genau einen Kanal nach draußen, den
//! Proxy-Socket. Über denselben Kanal erfährt er, wo er ist, und kann den
//! Menschen um etwas bitten: Der Proxy beantwortet den reservierten Host
//! `humanitl.internal` selbst — ohne Namensauflösung, ohne Upstream, ohne
//! Regelauswertung.
//!
//! | Pfad | Methode | Antwort |
//! |---|---|---|
//! | `/` | `GET` | Kurzstatus und die Regeln, die gerade gelten |
//! | `/why/<flow-id>` | `GET` | Entscheidung, Grund und Notiz zu einem Flow |
//! | `/ask` | `POST` | Freitext bis 2 KiB; erzeugt [`FlowEvent::AgentAsk`] |
//!
//! Ein unbekannter Pfad ist `404`, eine andere Methode auf einem bekannten Pfad
//! `405`. Der Endpunkt legt nie eine Regel an und entscheidet nie über einen
//! Flow: `/ask` ist eine Bitte, keine Aktion (ADR-014).
//!
//! # Was hier nicht hinausgeht
//!
//! Die Liste unter `/` zeigt Aktion, Methoden, Host, Pfad und Ablauf, sonst
//! nichts. Kein `note`, kein `created_from`, keine Regel-Id, keine Position:
//! Das sind Angaben des Menschen über seine eigene Arbeit, und der Agent hat
//! sie nicht zu lesen. Die Notiz unter `/why/<id>` ist die andere Notiz — die
//! aus der Entscheidung, die der Mensch ausdrücklich an den Agenten gerichtet
//! hat (`Decision::Block { note }`, HUM-072).
//!
//! # Was hier hereinkommt
//!
//! Der Body von `/ask` ist die einzige Stelle des ganzen Daemons, an der Text
//! vom Agenten angenommen wird. Er läuft durch dieselbe Säuberung wie die
//! Notiz des Menschen ([`sanitize_note`]): Zeilenumbrüche werden zu
//! Leerzeichen, Steuerzeichen und unsichtbare Zeichen fallen weg, und es
//! bleiben höchstens 500 Zeichen. Damit kann der Text weder eine Zeile der
//! Statusausgabe nachahmen noch im Terminal des Menschen etwas anderes
//! darstellen, als dort steht. Die Oberfläche zeigt ihn zusätzlich als
//! Klartext, nie als Markdown und nie als Verweis.

use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant, SystemTime};

use humanitl_config::AskMode;
use humanitl_core::rule::{Expiry, Matcher, Rule};
use humanitl_core::{
    AskId, Decision, FlowEvent, FlowId, HostName, Method, SessionId, sanitize_note,
};
use humanitl_rules::RuleSet;

use crate::registry::FlowRegistry;
use crate::session::SessionSettings;

// Der reservierte Name wohnt im Kern: Die Weiche hier und der Nachweis
// `MetaAnswer`, mit dem ein Fluss ohne Entscheidung aufgezeichnet wird, müssen
// denselben Namen meinen (HUM-103).
pub use humanitl_core::META_HOST;

/// So viele Bytes darf der Body von `/ask` haben.
pub const ASK_BODY_CAP_BYTES: u64 = 2 * 1024;

/// So viele Bitten nimmt eine Sitzung in einem Fenster an.
pub const ASK_PER_WINDOW: usize = 10;

/// Die Länge des Fensters, über das [`ASK_PER_WINDOW`] zählt.
pub const ASK_WINDOW: Duration = Duration::from_secs(60);

/// Wahr, wenn dieser Host der Meta-Host ist.
///
/// Verglichen wird der normalisierte [`HostName`], nicht der Text der Anfrage.
/// [`HostName::parse`] hat vorher aus `HUMANITL.INTERNAL` und
/// `humanitl.internal.` denselben Namen gemacht — es sind derselbe Name, und
/// beide gehören hierher. Ein Name, der nur so *aussieht*, gehört nicht
/// hierher: `evil-humanitl.internal`, `humanitl.internal.evil.io` und
/// `sub.humanitl.internal` sind eigene Namen und laufen durch die Regeln wie
/// jeder andere Host.
///
/// Der Port spielt keine Rolle: Reserviert ist der Name, nicht ein Dienst auf
/// einem Port (ADR-014, „wird nie aufgelöst"). Ginge `humanitl.internal:8080`
/// durch die Regeln, könnte eine Freigabe dafür eine Namensauflösung auslösen,
/// und genau das schließt der ADR aus.
#[must_use]
pub fn is_meta_host(host: &HostName) -> bool {
    host.is_meta()
}

/// Eine Uhr, die sich im Test stellen lässt.
///
/// Das Ratenlimit hängt an der Zeit, und ein Test, der auf die Wanduhr wartet,
/// wäre entweder langsam oder unzuverlässig. Der Daemon nimmt
/// [`SystemClock`], der Test eine eigene.
pub trait MetaClock: std::fmt::Debug + Send + Sync {
    /// Der aktuelle Zeitpunkt auf der monotonen Uhr.
    fn now(&self) -> Instant;
}

/// Die monotone Uhr des Systems.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl MetaClock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Was `/` über die Sitzung sagt.
#[derive(Debug, Clone)]
pub struct MetaStatus {
    /// Wo gefragt wird (`hold.ask_mode`).
    pub ask_mode: AskMode,
    /// Wie lange eine gehaltene Anfrage wartet (`hold.timeout_secs`).
    pub hold_timeout: Duration,
    /// Der Endpunkt des Sprachmodells als `host:port`, falls einer
    /// eingestellt ist (`llm.endpoint`).
    pub llm: Option<String>,
}

impl MetaStatus {
    /// Ein Status ohne Sprachmodell, mit den Vorgaben der Konfiguration.
    #[must_use]
    pub const fn new(ask_mode: AskMode, hold_timeout: Duration) -> Self {
        Self {
            ask_mode,
            hold_timeout,
            llm: None,
        }
    }

    /// Derselbe Status mit einem Sprachmodell-Endpunkt.
    #[must_use]
    pub fn with_llm(mut self, llm: impl Into<String>) -> Self {
        self.llm = Some(llm.into());
        self
    }
}

/// Die Anfrage, wie der Handler sie dem Endpunkt übergibt.
#[derive(Debug, Clone, Copy)]
pub struct MetaRequest<'a> {
    /// Die Methode der Anfrage.
    pub method: &'a Method,
    /// Pfad samt Query, so wie er in der Anfragezeile stand.
    pub path_and_query: &'a str,
    /// Der gepufferte Body; leer bei `GET`.
    pub body: &'a [u8],
    /// Wahr, wenn der Body über [`ASK_BODY_CAP_BYTES`] lag und deshalb nicht
    /// vollständig gelesen wurde.
    pub body_over_cap: bool,
    /// Die Sitzung der Verbindung; sie begrenzt `/why` und trägt das
    /// Ratenlimit.
    pub session: SessionId,
}

/// Die Antwort des Endpunkts, noch ohne HTTP-Verpackung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaReply {
    /// Der HTTP-Status.
    pub status: u16,
    /// Der Body, `text/plain`.
    pub body: String,
    /// Kopfzeilen über `Content-Type` hinaus, etwa `Allow` oder `Retry-After`.
    pub headers: Vec<(&'static str, String)>,
}

/// Was der Endpunkt beantwortet hat, samt dem Ereignis, das dabei entstand.
///
/// Der Endpunkt veröffentlicht nichts selbst: Er kennt die Warteschlange
/// nicht, und ein Test kann so das Ereignis prüfen, ohne einen Proxy zu
/// starten. Der Handler nimmt [`MetaOutcome::event`] und schiebt es in den
/// Strom.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaOutcome {
    /// Die Antwort an den Agenten.
    pub reply: MetaReply,
    /// Das Ereignis, das der Aufrufer veröffentlicht; nur bei `/ask`.
    pub event: Option<FlowEvent>,
}

/// Der Endpunkt selbst: Status, Regeln, Uhr und das Ratenlimit je Sitzung.
#[derive(Debug)]
pub struct MetaEndpoint {
    status: MetaStatus,
    /// Der Stand der laufenden Sitzung, falls einer geführt wird.
    ///
    /// Er geht vor [`MetaEndpoint::status`]: Der Endpunkt beantwortet die
    /// Frage „was gilt gerade", und was beim Bau des Endpunkts galt, ist nach
    /// dem Start einer Sitzung nicht mehr dasselbe (HUM-067). Ohne
    /// Sitzungszustand — in Tests und im Fake-Modus — bleibt es beim Wert des
    /// Baus.
    settings: Option<Arc<SessionSettings>>,
    rules: Arc<RwLock<RuleSet>>,
    clock: Arc<dyn MetaClock>,
    /// Die Zeitpunkte der angenommenen Bitten je Sitzung, älteste zuerst.
    asks: Mutex<HashMap<SessionId, VecDeque<Instant>>>,
}

impl MetaEndpoint {
    /// Ein Endpunkt über diesem Regelsatz.
    ///
    /// `rules` ist dasselbe Handle, das die
    /// [`RulesPipeline`](crate::pipeline::RulesPipeline) liest
    /// ([`RulesStore::snapshot`](crate::rules_store::RulesStore::snapshot)):
    /// Die Liste unter `/` zeigt damit denselben Satz, nach dem entschieden
    /// wird, und nicht eine zweite Kopie, die auseinanderlaufen könnte.
    #[must_use]
    pub fn new(status: MetaStatus, rules: Arc<RwLock<RuleSet>>) -> Self {
        Self {
            status,
            settings: None,
            rules,
            clock: Arc::new(SystemClock),
            asks: Mutex::new(HashMap::new()),
        }
    }

    /// Derselbe Endpunkt mit einer anderen Uhr (Test).
    #[must_use]
    pub fn with_clock(mut self, clock: Arc<dyn MetaClock>) -> Self {
        self.clock = clock;
        self
    }

    /// Derselbe Endpunkt, der seinen Status aus der laufenden Sitzung liest.
    #[must_use]
    pub fn with_settings(mut self, settings: Arc<SessionSettings>) -> Self {
        self.settings = Some(settings);
        self
    }

    /// Der Stand, den `/` zeigt: der der Sitzung, sonst der des Baus.
    fn status(&self) -> MetaStatus {
        match self.settings.as_ref() {
            Some(settings) => {
                let state = settings.get();
                MetaStatus {
                    ask_mode: state.ask_mode,
                    hold_timeout: state.hold_timeout,
                    llm: state.llm,
                }
            }
            None => self.status.clone(),
        }
    }

    /// Wie viele Sitzungen gerade ein Fenster des Ratenlimits haben.
    ///
    /// Die Zahl fällt auf null zurück, sobald die letzte Bitte einer Sitzung
    /// aus ihrem Fenster gelaufen ist und irgendjemand wieder fragt: Das
    /// Aufräumen hängt am Annehmen einer Bitte, nicht an einem
    /// Zeitgeber. Ohne diese Auskunft ließe sich nicht prüfen, dass die
    /// Tabelle über die Laufzeit des Daemons nicht wächst.
    #[must_use]
    pub fn tracked_sessions(&self) -> usize {
        self.asks
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Beantwortet eine Anfrage an den Meta-Host.
    ///
    /// `registry` liefert die Auskunft für `/why/<flow-id>`; nur Flows der
    /// Sitzung aus `request` werden beantwortet, jeder andere Flow ist `404`.
    /// Ein Agent soll nicht erfahren, was in einer anderen Sitzung geschah.
    #[must_use]
    pub fn respond(&self, request: &MetaRequest<'_>, registry: &FlowRegistry) -> MetaOutcome {
        let path = path_only(request.path_and_query);
        match route(path) {
            Some(MetaRoute::Status) => {
                guard(request, &Method::GET, || self.status_body(request.session))
            }
            Some(MetaRoute::Why(flow)) => {
                guard(request, &Method::GET, || why_body(flow, request, registry))
            }
            Some(MetaRoute::Ask) => guard(request, &Method::POST, || self.ask(request)),
            None => MetaOutcome {
                reply: text(404, "not found\n"),
                event: None,
            },
        }
    }

    /// Der Kurzstatus und die Regeln, die gerade gelten.
    fn status_body(&self, session: SessionId) -> MetaOutcome {
        let status = self.status();
        let mut out = format!(
            "humanitl session={session} ask={ask} timeout={timeout} llm={llm}\n",
            ask = ask_mode_name(status.ask_mode),
            timeout = status.hold_timeout.as_secs(),
            llm = status.llm.as_deref().unwrap_or("none"),
        );
        out.push_str("rules (first match wins):\n");
        let rows = self.rule_rows(session);
        let widths = column_widths(&rows);
        for row in &rows {
            row.write(&mut out, &widths);
        }
        MetaOutcome {
            reply: text(200, &out),
            event: None,
        }
    }

    /// Die Regeln in Auswertungsreihenfolge, als fertige Zeilen.
    ///
    /// Dieselbe Reihenfolge wie in
    /// [`RuleSet::evaluate`](humanitl_rules::RuleSet::evaluate): erst die
    /// Regeln dieser Sitzung, dann alle übrigen, und in beiden Durchgängen
    /// gewinnt die erste passende. Abgelaufene und abgeschaltete Regeln
    /// entscheiden nichts und stehen deshalb auch nicht in der Liste; sie dort
    /// zu zeigen, hieße mehr zu behaupten, als wahr ist
    /// (`backlog/CONVENTIONS.md` 4.13).
    ///
    /// Die letzte Zeile ist der Ausgang ohne Treffer: `ask` über alles.
    fn rule_rows(&self, session: SessionId) -> Vec<RuleRow> {
        let now = chrono::Utc::now();
        let mut rows = Vec::new();
        let Ok(rules) = self.rules.read() else {
            // Ein vergifteter Regelsatz ist kein Grund, dem Agenten eine
            // Liste zu erfinden. Es bleibt die Vorgabezeile: gefragt wird.
            tracing::error!("the rule set is poisoned; the status page shows the default only");
            rows.push(RuleRow::default_row());
            return rows;
        };
        for session_scoped in [true, false] {
            for rule in rules.iter() {
                if matches!(rule.expires, Expiry::Session(_)) != session_scoped {
                    continue;
                }
                if rule.disabled || rule.expires.is_expired(now, session) {
                    continue;
                }
                rows.push(RuleRow::of(rule));
            }
        }
        rows.push(RuleRow::default_row());
        rows
    }

    /// `/ask`: Text annehmen, säubern, als Ereignis melden.
    ///
    /// Die Reihenfolge der Prüfungen ist Absicht. Zuerst die Länge des
    /// Rumpfes: Sie steht schon fest, bevor irgendetwas gelesen wurde. Dann
    /// der Inhalt. **Erst zuletzt der Platz im Fenster**, denn nur eine
    /// angenommene Bitte belegt einen; verbrauchte eine leere Bitte einen
    /// Platz, sperrte sich ein Agent mit zehn kaputten Rümpfen selbst aus,
    /// und das Ratenlimit wäre eine Waffe gegen den, den es schützen soll.
    fn ask(&self, request: &MetaRequest<'_>) -> MetaOutcome {
        if request.body_over_cap {
            return MetaOutcome {
                reply: text(413, &format!("ask body over {ASK_BODY_CAP_BYTES} bytes\n")),
                event: None,
            };
        }
        // Genau die Säuberung der Block-Notiz (HUM-072). Eine zweite Fassung
        // für denselben Zweck liefe irgendwann auseinander, und die
        // schwächere von beiden wäre dann das Loch.
        let text_of_ask = sanitize_note(&String::from_utf8_lossy(request.body));
        if text_of_ask.is_empty() {
            return MetaOutcome {
                reply: text(400, "empty ask\n"),
                event: None,
            };
        }
        if let Err(retry_after) = self.take_ask_slot(request.session) {
            let mut reply = text(
                429,
                &format!(
                    "rate limited: at most {ASK_PER_WINDOW} asks per {window} seconds\n",
                    window = ASK_WINDOW.as_secs()
                ),
            );
            reply
                .headers
                .push(("retry-after", retry_after.as_secs().max(1).to_string()));
            return MetaOutcome { reply, event: None };
        }
        let target = suggested_target(&text_of_ask);
        MetaOutcome {
            reply: text(202, "queued\n"),
            event: Some(FlowEvent::AgentAsk {
                ask_id: AskId::new(),
                at: SystemTime::now(),
                text: text_of_ask,
                suggested_host: target.as_ref().map(|found| found.host.clone()),
                suggested_path: target.and_then(|found| found.path),
            }),
        }
    }

    /// Nimmt einen Platz im Fenster dieser Sitzung, wenn noch einer frei ist.
    ///
    /// **Gleitendes Fenster, kein fester Minutenzähler.** Ein fester Zähler,
    /// der zur vollen Minute auf null springt, lässt zwanzig Bitten in zwei
    /// Sekunden durch, wenn sie um die Grenze herum liegen — genau das
    /// Verhalten, das die Grenze verhindern soll. Zehn Zeitpunkte je Sitzung
    /// sind so wenig Speicher, dass der genauere Weg nichts kostet.
    ///
    /// Nur angenommene Bitten belegen einen Platz. Eine abgelehnte mitzuzählen
    /// hieße, dass ein Agent, der einmal zu schnell war, sich selbst dauerhaft
    /// aussperrt.
    ///
    /// # Errors
    ///
    /// Die Wartezeit, bis wieder ein Platz frei wird.
    fn take_ask_slot(&self, session: SessionId) -> Result<(), Duration> {
        let now = self.clock.now();
        let mut asks = self.asks.lock().unwrap_or_else(PoisonError::into_inner);
        // Jedes Fenster gegen `now` beschneiden und danach die leeren
        // wegwerfen — in dieser Reihenfolge. Andersherum bliebe das Fenster
        // einer Sitzung, die nicht mehr fragt, für immer nicht-leer, und die
        // Tabelle wüchse über die Laufzeit des Daemons mit jeder Sitzung.
        asks.retain(|_, window| {
            prune_window(window, now);
            !window.is_empty()
        });
        let window = asks.entry(session).or_default();
        prune_window(window, now);
        if window.len() >= ASK_PER_WINDOW {
            let waited = window
                .front()
                .map_or(Duration::ZERO, |at| now.saturating_duration_since(*at));
            return Err(ASK_WINDOW.saturating_sub(waited));
        }
        window.push_back(now);
        Ok(())
    }
}

/// Die drei Pfade, die der Endpunkt kennt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MetaRoute {
    /// `/`
    Status,
    /// `/why/<flow-id>`
    Why(FlowId),
    /// `/ask`
    Ask,
}

/// Wirft aus einem Fenster alles, was älter ist als [`ASK_WINDOW`].
///
/// Die Zeitpunkte stehen aufsteigend darin, also genügt es, vorn abzutragen.
fn prune_window(window: &mut VecDeque<Instant>, now: Instant) {
    while window
        .front()
        .is_some_and(|at| now.saturating_duration_since(*at) >= ASK_WINDOW)
    {
        window.pop_front();
    }
}

/// Prüft die Methode und ruft sonst den Rumpf.
///
/// Der Pfad entscheidet vor der Methode: Ein unbekannter Pfad ist `404`, auch
/// mit `POST`; erst auf einem bekannten Pfad ist eine andere Methode `405`.
/// Andersherum verriete die Antwort, welche Pfade es gibt.
fn guard(
    request: &MetaRequest<'_>,
    allowed: &Method,
    body: impl FnOnce() -> MetaOutcome,
) -> MetaOutcome {
    if request.method == allowed {
        return body();
    }
    let mut reply = text(405, "method not allowed\n");
    reply.headers.push(("allow", allowed.as_str().to_owned()));
    MetaOutcome { reply, event: None }
}

/// Der Pfad ohne Query.
fn path_only(path_and_query: &str) -> &str {
    let end = path_and_query
        .find(['?', '#'])
        .unwrap_or(path_and_query.len());
    &path_and_query[..end]
}

/// Ordnet einen Pfad einer Route zu; `None` ist `404`.
///
/// Der Vergleich ist genau: kein Präfix, keine Groß-/Kleinschreibung, kein
/// abschließender Schrägstrich als Synonym. `/ask/` ist nicht `/ask`, und
/// `/why/<id>/mehr` ist keine Flow-Id.
fn route(path: &str) -> Option<MetaRoute> {
    match path {
        "/" => Some(MetaRoute::Status),
        "/ask" => Some(MetaRoute::Ask),
        _ => path
            .strip_prefix("/why/")
            .and_then(|id| FlowId::parse(id).ok())
            .map(MetaRoute::Why),
    }
}

/// `/why/<flow-id>`: die Entscheidung zu einem Flow dieser Sitzung.
fn why_body(flow: FlowId, request: &MetaRequest<'_>, registry: &FlowRegistry) -> MetaOutcome {
    // Ein Flow einer fremden Sitzung wird behandelt, als gäbe es ihn nicht:
    // Die Antwort darf nicht verraten, dass er existiert.
    let Some(record) = registry
        .get(flow)
        .filter(|record| record.session == request.session)
    else {
        return MetaOutcome {
            reply: text(404, "no such flow\n"),
            event: None,
        };
    };
    let (decision, reason, note) = match &record.decision {
        // Noch nicht entschieden: Der Zustand ist die ganze Auskunft. Der
        // Agent wartet ohnehin gerade auf genau diese Antwort.
        None => (
            "pending".to_owned(),
            record.state.name().to_owned(),
            String::new(),
        ),
        Some(Decision::Block { reason, note }) => (
            "block".to_owned(),
            reason.as_str().to_owned(),
            note.as_deref().map(sanitize_note).unwrap_or_default(),
        ),
        Some(Decision::TimedOut) => ("timed_out".to_owned(), "timeout".to_owned(), String::new()),
        Some(decision @ (Decision::Allow | Decision::AllowEdited { .. })) => (
            decision.as_str().to_owned(),
            record
                .decision_source
                .map_or("unknown", humanitl_core::DecisionSource::as_str)
                .to_owned(),
            String::new(),
        ),
    };
    // `note` steht am Ende der Zeile, weil es das einzige Feld mit
    // Leerzeichen ist: So bleibt die Zeile für den Agenten zerlegbar.
    MetaOutcome {
        reply: text(
            200,
            &format!("decision={decision} reason={reason} note={note}\n"),
        ),
        event: None,
    }
}

/// Eine Zeile der Regel-Liste unter `/`.
///
/// Was hier fehlt, ist der Punkt der Struktur: keine Id, keine Notiz, keine
/// Herkunft aus einer Entscheidung, keine Position. Wer eine Spalte
/// hinzufügt, fügt sie dem Agenten hinzu.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RuleRow {
    action: String,
    methods: String,
    host: String,
    path: String,
    expiry: String,
    tags: Vec<&'static str>,
}

impl RuleRow {
    /// Die Zeile einer Regel.
    fn of(rule: &Rule) -> Self {
        let mut tags = Vec::new();
        if rule.passthrough_llm {
            tags.push("llm passthrough");
        }
        if rule.bundled {
            tags.push("bundled");
        }
        Self {
            action: cell(rule.action.as_str()),
            methods: cell(&methods_of(&rule.matcher)),
            host: cell(&host_of(&rule.matcher)),
            path: cell(&path_of(&rule.matcher)),
            expiry: cell(&expiry_of(rule.expires)),
            tags,
        }
    }

    /// Der Ausgang ohne Treffer: Es wird gefragt.
    fn default_row() -> Self {
        Self {
            action: "ask".to_owned(),
            methods: "*".to_owned(),
            host: "*".to_owned(),
            path: "*".to_owned(),
            expiry: "default".to_owned(),
            tags: Vec::new(),
        }
    }

    /// Schreibt die Zeile mit den Spaltenbreiten aus [`column_widths`].
    fn write(&self, out: &mut String, widths: &[usize; 4]) {
        // `write!` in einen `String` kann nicht scheitern; das Ergebnis
        // interessiert deshalb niemanden, und ein `unwrap` wäre hier falsch.
        let _ = write!(
            out,
            "  {action:<aw$}  {methods:<mw$}  {host:<hw$}  {path:<pw$}  {expiry}",
            action = self.action,
            methods = self.methods,
            host = self.host,
            path = self.path,
            expiry = self.expiry,
            aw = widths[0],
            mw = widths[1],
            hw = widths[2],
            pw = widths[3],
        );
        if !self.tags.is_empty() {
            let _ = write!(out, "  ({})", self.tags.join(", "));
        }
        out.push('\n');
    }
}

/// Die Breite der vier linken Spalten, damit die Liste eine Tabelle bleibt.
///
/// Gekürzt wird nichts: Ein abgeschnittener Host wäre ein anderer Host.
fn column_widths(rows: &[RuleRow]) -> [usize; 4] {
    let mut widths = [6_usize, 7, 18, 6];
    for row in rows {
        for (slot, cell) in widths
            .iter_mut()
            .zip([&row.action, &row.methods, &row.host, &row.path])
        {
            *slot = (*slot).max(cell.chars().count());
        }
    }
    widths
}

/// Ein Feld der Liste, gesäubert wie eine Notiz.
///
/// Muster aus `rules.yaml` sind Text aus einer Datei; ein Zeilenumbruch darin
/// könnte eine zweite Zeile in die Statusausgabe schreiben und dem Agenten
/// eine Regel vorspielen, die es nicht gibt. Leer bleibt nichts: ein leeres
/// Feld wäre eine Lücke ohne Bedeutung.
fn cell(text: &str) -> String {
    let cleaned = sanitize_note(text);
    if cleaned.is_empty() {
        "?".to_owned()
    } else {
        cleaned
    }
}

/// Die Methoden einer Bedingung; `*` heißt jede.
fn methods_of(matcher: &Matcher) -> String {
    match &matcher.methods {
        None => "*".to_owned(),
        Some(methods) if methods.is_empty() => "*".to_owned(),
        Some(methods) => methods
            .iter()
            .map(Method::as_str)
            .collect::<Vec<_>>()
            .join(","),
    }
}

/// Der Host einer Bedingung, mit Port, falls die Regel einen verlangt.
fn host_of(matcher: &Matcher) -> String {
    match matcher.port {
        Some(port) => format!("{host}:{port}", host = matcher.host),
        None => matcher.host.to_string(),
    }
}

/// Der Pfad einer Bedingung; `*` heißt jeder.
///
/// Ein Präfix wird als `<präfix>**` geschrieben, weil es genau das bedeutet
/// und weil ein Agent die Schreibweise aus `rules.yaml` wiedererkennt.
fn path_of(matcher: &Matcher) -> String {
    let mut parts = Vec::new();
    if let Some(path) = &matcher.path {
        parts.push(path.to_string());
    }
    for prefix in &matcher.path_prefixes {
        parts.push(format!("{prefix}**"));
    }
    if parts.is_empty() {
        "*".to_owned()
    } else {
        parts.join(",")
    }
}

/// Die Gültigkeit einer Regel.
fn expiry_of(expires: Expiry) -> String {
    match expires {
        Expiry::Never => "never".to_owned(),
        Expiry::Session(_) => "session".to_owned(),
        Expiry::At(at) => format!("until {}", at.to_rfc3339()),
    }
}

/// Der Name des Ask-Modus, wie ihn `/` zeigt.
fn ask_mode_name(mode: AskMode) -> &'static str {
    match mode {
        AskMode::Ui => "ui",
        AskMode::Terminal => "terminal",
        AskMode::None => "none",
    }
}

/// Eine `text/plain`-Antwort.
fn text(status: u16, body: &str) -> MetaReply {
    MetaReply {
        status,
        body: body.to_owned(),
        headers: Vec::new(),
    }
}

/// Was aus der ersten URL im Text als Regel-Vorschlag taugt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestedTarget {
    /// Der Host, normalisiert.
    pub host: String,
    /// Der Pfad ohne Query und Fragment, falls er als Muster taugt.
    ///
    /// `None` heißt: Die URL nannte keinen Pfad, oder ihr Pfad ist nichts,
    /// worauf eine Regel zeigen sollte. Der Unterschied ist wichtig — eine
    /// Regel ohne Pfad öffnet den ganzen Host, und die Oberfläche muss das
    /// sagen können.
    pub path: Option<String>,
}

/// So lang darf ein vorgeschlagener Pfad höchstens sein.
const SUGGESTED_PATH_MAX: usize = 200;

/// Host und Pfad aus der **ersten** URL des Textes, falls sie taugen.
///
/// Der Text kommt vom Agenten. Er wird deshalb nicht „verstanden", sondern nur
/// nach genau einem Muster durchsucht, und was dabei herauskommt, läuft durch
/// [`HostName::parse`], bevor es irgendwo als Host gilt. Regeln:
///
/// 1. Gesucht wird `http://` oder `https://`, ohne Rücksicht auf
///    Groß-/Kleinschreibung. Ohne Schema kein Vorschlag: `example.com` in
///    einem Satz ist ein Wort, keine Adresse.
/// 2. Es zählt die **erste** Fundstelle. Steht dahinter Unsinn, gibt es keinen
///    Vorschlag — es wird nicht weitergesucht. Sonst könnte ein Text mit einer
///    kaputten ersten und einer gültigen zweiten URL den Blick des Menschen
///    auf die eine und den Vorschlag auf die andere lenken.
/// 3. Benutzerangaben vor einem `@` fallen weg (`https://github.com@evil.io/`
///    zeigt auf `evil.io`, und genau das wird vorgeschlagen).
/// 4. Ein Port fällt weg; die Regel entsteht über den Host.
/// 5. Vorgeschlagen wird nur ein DNS-Name mit mindestens einem Punkt. Eine
///    Adresse, ein Name aus einem einzigen Label (`https://ein Satz` liest
///    sich als Host `ein`) und `humanitl.internal` selbst ergeben keinen
///    Vorschlag.
/// 6. Der Pfad kommt aus derselben URL, ohne Query und Fragment, und nur wenn
///    er die Prüfung des Pfad-Musters übersteht. Er ist der Grund, warum es diese
///    Funktion überhaupt gibt: Ohne ihn wäre die vorgeschlagene Regel eine
///    Freigabe für **jeden** Pfad des Hosts, während der Agent nach genau
///    einer Adresse gefragt hat.
#[must_use]
pub fn suggested_target(text: &str) -> Option<SuggestedTarget> {
    let bytes = text.as_bytes();
    let start = (0..bytes.len()).find(|&index| {
        starts_with_ignore_ascii_case(&bytes[index..], b"http://")
            || starts_with_ignore_ascii_case(&bytes[index..], b"https://")
    })?;
    let rest = &text[start..];
    let after_scheme = rest.find("//").map(|at| at + 2)?;
    let tail = &rest[after_scheme..];
    let end = tail
        .find(|ch: char| {
            matches!(
                ch,
                '/' | '?' | '#' | '\\' | '"' | '\'' | '<' | '>' | '|' | '^' | '`'
            ) || ch.is_whitespace()
        })
        .unwrap_or(tail.len());
    let authority = &tail[..end];
    // Benutzerangaben: alles bis zum letzten `@` gehört nicht zum Host.
    let host_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match HostName::parse(strip_port(host_port)).ok()? {
        HostName::Dns(name) if name != META_HOST && name.contains('.') => name,
        HostName::Dns(_) | HostName::Ip(_) => return None,
    };
    Some(SuggestedTarget {
        host,
        path: suggested_path(&tail[end..]),
    })
}

/// Der Pfad einer URL als Muster für eine Regel, falls er dafür taugt.
///
/// `rest` ist alles ab dem `/`, das die Authority beendet. Was hier
/// herauskommt, wird in der Oberfläche als [`PathPattern`] vorgeschlagen, und
/// ein Muster aus feindlichem Text muss deshalb strenger geprüft werden als
/// ein Pfad, den nur jemand lesen soll:
///
/// * Query und Fragment fallen weg. Ein Glob über Pfad **und** Query
///   (`RequestKey` vergleicht beides) aus dem Text des Agenten wäre eine
///   Bedingung, die niemand gelesen hat.
/// * `*` fällt aus: Es ist der Platzhalter der Muster-Sprache. Ein `*` aus
///   dem Text des Agenten machte aus dem engen Vorschlag einen weiten.
/// * Alles außerhalb der sichtbaren ASCII-Zeichen fällt aus. Ein Client
///   schreibt einen Pfad prozentkodiert; steht dort etwas anderes, ist es
///   kein Pfad, sondern Prosa.
/// * `/` allein und ein leerer Rest ergeben `None`: Das ist kein engerer
///   Vorschlag als „der ganze Host", und die Oberfläche soll den Unterschied
///   zeigen, statt ihn zu verwischen.
fn suggested_path(rest: &str) -> Option<String> {
    // Die URL endet am ersten Zeichen, das nicht mehr zu ihr gehört: Query,
    // Fragment, Leerraum oder eines der Zeichen, die eine URL in Prosa
    // begrenzen. Ohne diesen Schnitt trüge der Pfad den halben Satz mit sich.
    let end = rest
        .find(|ch: char| {
            matches!(
                ch,
                '?' | '#' | '\\' | '"' | '\'' | '<' | '>' | '|' | '^' | '`'
            ) || ch.is_whitespace()
        })
        .unwrap_or(rest.len());
    let path = &rest[..end];
    if path.len() < 2 || !path.starts_with('/') || path.len() > SUGGESTED_PATH_MAX {
        return None;
    }
    path.chars()
        .all(|ch| matches!(ch, '!'..='~') && ch != '*')
        .then(|| path.to_owned())
}

/// Wahr, wenn `haystack` mit `needle` beginnt, ASCII-Fall egal.
///
/// `needle` ist reines ASCII; ein Treffer kann deshalb nur an einer
/// Zeichengrenze beginnen, und der Schnitt in den Text bleibt gültig.
fn starts_with_ignore_ascii_case(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && haystack[..needle.len()].eq_ignore_ascii_case(needle)
}

/// Trennt den Port ab; eine IPv6-Adresse in Klammern bleibt heil.
fn strip_port(host_port: &str) -> &str {
    if host_port.starts_with('[') {
        return host_port
            .find(']')
            .map_or(host_port, |close| &host_port[..=close]);
    }
    host_port
        .rsplit_once(':')
        .map_or(host_port, |(host, _port)| host)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::atomic::{AtomicU64, Ordering};

    use humanitl_config::Limits;
    use humanitl_core::rule::{Action, HostPattern, PathPattern};
    use humanitl_core::{
        Authority, BlockReason, DecisionSource, Flow, FlowId, HttpRequest, RuleId, Scheme,
    };

    use super::{
        ASK_PER_WINDOW, ASK_WINDOW, MetaClock, MetaEndpoint, MetaOutcome, MetaRequest, MetaStatus,
        is_meta_host, suggested_target,
    };
    use crate::connect::ConnectionContext;
    use crate::registry::{FlowRecord, FlowRegistry};
    use humanitl_core::{FlowEvent, HostName, Method, SessionId};
    use humanitl_rules::RuleSet;
    use std::sync::{Arc, RwLock};
    use std::time::{Duration, Instant, SystemTime};

    /// Eine Uhr, die nur vorwärts geht, wenn ein Test sie schiebt.
    #[derive(Debug)]
    struct ManualClock {
        base: Instant,
        offset_ms: AtomicU64,
    }

    impl ManualClock {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                base: Instant::now(),
                offset_ms: AtomicU64::new(0),
            })
        }

        fn advance(&self, by: Duration) {
            let millis = u64::try_from(by.as_millis()).unwrap_or(u64::MAX);
            self.offset_ms.fetch_add(millis, Ordering::Relaxed);
        }
    }

    impl MetaClock for ManualClock {
        fn now(&self) -> Instant {
            self.base
                .checked_add(Duration::from_millis(
                    self.offset_ms.load(Ordering::Relaxed),
                ))
                .unwrap_or(self.base)
        }
    }

    /// Der Host aus [`suggested_target`], für die Tests, die nur ihn prüfen.
    fn suggested_host(text: &str) -> Option<String> {
        suggested_target(text).map(|found| found.host)
    }

    /// Der Pfad aus [`suggested_target`].
    fn suggested_path_of(text: &str) -> Option<String> {
        suggested_target(text).and_then(|found| found.path)
    }

    fn endpoint(rules: RuleSet) -> MetaEndpoint {
        MetaEndpoint::new(
            MetaStatus::new(humanitl_config::AskMode::Ui, Duration::from_secs(300))
                .with_llm("192.168.1.50:11434"),
            Arc::new(RwLock::new(rules)),
        )
    }

    fn registry() -> FlowRegistry {
        FlowRegistry::new(&Limits::default())
    }

    fn get(path: &str, session: SessionId) -> MetaRequest<'static> {
        // Der Pfad lebt so lange wie der Test; `leak` hält ihn fest, damit die
        // Hilfsfunktion keine Lebensdauer durchreichen muss.
        MetaRequest {
            method: &Method::GET,
            path_and_query: Box::leak(path.to_owned().into_boxed_str()),
            body: b"",
            body_over_cap: false,
            session,
        }
    }

    fn post<'a>(path: &'a str, body: &'a [u8], session: SessionId) -> MetaRequest<'a> {
        MetaRequest {
            method: &Method::POST,
            path_and_query: path,
            body,
            body_over_cap: false,
            session,
        }
    }

    fn rule(action: Action, host: &str) -> humanitl_core::Rule {
        humanitl_core::Rule::new(
            RuleId::new(),
            action,
            humanitl_core::Matcher::host(HostPattern::parse(host).unwrap()),
        )
    }

    // -----------------------------------------------------------------
    // `/`
    // -----------------------------------------------------------------

    #[test]
    fn status_lists_effective_rules() {
        let session = SessionId::new();
        let persistent = rule(Action::Block, "models.dev");
        let session_rule =
            rule(Action::Allow, "*.npmjs.org").with_expiry(humanitl_core::Expiry::Session(session));
        // Die Sitzungsregel steht *hinter* der dauerhaften im Speicher und
        // muss trotzdem vor ihr stehen: Genau so wertet `RuleSet::evaluate`
        // aus (`backlog/CONVENTIONS.md` 4.5).
        let set = RuleSet::from_rules(vec![persistent, session_rule]);
        let out = endpoint(set).respond(&get("/", session), &registry());

        assert_eq!(out.reply.status, 200);
        assert_eq!(out.event, None);
        let lines: Vec<&str> = out.reply.body.lines().collect();
        assert!(
            lines[0].starts_with(&format!(
                "humanitl session={session} ask=ui timeout=300 llm="
            )),
            "{}",
            lines[0]
        );
        assert!(lines[0].ends_with("llm=192.168.1.50:11434"), "{}", lines[0]);
        assert_eq!(lines[1], "rules (first match wins):");
        assert!(lines[2].contains("*.npmjs.org"), "{}", lines[2]);
        assert!(lines[2].contains("session"), "{}", lines[2]);
        assert!(lines[3].contains("models.dev"), "{}", lines[3]);
        assert!(lines[4].starts_with("  ask"), "{}", lines[4]);
        assert!(lines[4].ends_with("default"), "{}", lines[4]);
        assert_eq!(lines.len(), 5);
    }

    #[test]
    fn status_never_shows_what_the_human_wrote() {
        let session = SessionId::new();
        let secret_note = "weil der Kunde Meier heisst";
        let origin = FlowId::new();
        let noted = rule(Action::Allow, "api.github.com")
            .with_note(secret_note)
            .created_from(origin);
        let out =
            endpoint(RuleSet::from_rules(vec![noted])).respond(&get("/", session), &registry());

        let body = out.reply.body;
        assert!(body.contains("api.github.com"), "{body}");
        assert!(
            !body.contains(secret_note),
            "the note of a rule is the human's own writing: {body}"
        );
        assert!(
            !body.contains(&origin.to_string()),
            "created_from names a flow the agent has no business with: {body}"
        );
    }

    #[test]
    fn status_skips_rules_that_decide_nothing() {
        let session = SessionId::new();
        let disabled = rule(Action::Block, "disabled.example").disabled(true);
        let foreign = rule(Action::Allow, "foreign.example")
            .with_expiry(humanitl_core::Expiry::Session(SessionId::new()));
        let expired = rule(Action::Allow, "expired.example").with_expiry(
            humanitl_core::Expiry::At(chrono::Utc::now() - chrono::Duration::hours(1)),
        );
        let out = endpoint(RuleSet::from_rules(vec![disabled, foreign, expired]))
            .respond(&get("/", session), &registry());

        let body = out.reply.body;
        assert!(!body.contains("disabled.example"), "{body}");
        assert!(!body.contains("foreign.example"), "{body}");
        assert!(!body.contains("expired.example"), "{body}");
        assert_eq!(body.lines().count(), 3, "{body}");
    }

    #[test]
    fn a_rule_pattern_cannot_write_a_second_line() {
        let session = SessionId::new();
        let sneaky = humanitl_core::Rule::new(
            RuleId::new(),
            Action::Allow,
            humanitl_core::Matcher::host(HostPattern::parse("api.github.com").unwrap())
                .with_path(PathPattern::parse("/a\n  allow   *   evil.io   *   never")),
        );
        let out =
            endpoint(RuleSet::from_rules(vec![sneaky])).respond(&get("/", session), &registry());

        // Zwei Regelzeilen plus Kopf, Überschrift und Vorgabezeile wären
        // fünf; es dürfen vier sein.
        assert_eq!(out.reply.body.lines().count(), 4, "{}", out.reply.body);
        assert!(!out.reply.body.contains('\r'));
    }

    #[test]
    fn status_shows_the_passthrough_and_the_bundled_tag() {
        let session = SessionId::new();
        let passthrough = humanitl_core::Rule::new(
            RuleId::new(),
            Action::Allow,
            humanitl_core::Matcher::host(HostPattern::parse("ip:192.168.1.50").unwrap())
                .with_port(11434)
                .with_methods(vec![Method::POST])
                .with_path_prefixes(vec!["/v1/".to_owned()]),
        )
        .passthrough_llm(true);
        let bundled = rule(Action::Block, "models.dev").bundled(true);
        let out = endpoint(RuleSet::from_rules(vec![passthrough, bundled]))
            .respond(&get("/", session), &registry());

        let body = out.reply.body;
        // Die Schreibweise ist die von `rules.yaml`: `ip:` gehört zum Muster,
        // sonst wäre die Zeile nicht mehr als Regel zu lesen.
        assert!(body.contains("ip:192.168.1.50:11434"), "{body}");
        assert!(body.contains("/v1/**"), "{body}");
        assert!(body.contains("(llm passthrough)"), "{body}");
        assert!(body.contains("(bundled)"), "{body}");
        assert!(body.contains("POST"), "{body}");
    }

    #[test]
    fn post_root_405() {
        let session = SessionId::new();
        let out = endpoint(RuleSet::new()).respond(&post("/", b"x", session), &registry());
        assert_eq!(out.reply.status, 405);
        assert_eq!(
            out.reply.headers,
            vec![("allow", "GET".to_owned())],
            "the answer says which method would work"
        );
        assert_eq!(out.event, None);
    }

    #[test]
    fn unknown_paths_are_404_whatever_the_method() {
        let session = SessionId::new();
        let end = endpoint(RuleSet::new());
        for path in ["/nope", "/ask/", "/why", "/why/", "/Ask", "/index.html", ""] {
            let out = end.respond(&get(path, session), &registry());
            assert_eq!(out.reply.status, 404, "GET {path}");
            let out = end.respond(&post(path, b"x", session), &registry());
            assert_eq!(out.reply.status, 404, "POST {path}");
        }
    }

    // -----------------------------------------------------------------
    // `/why/<id>`
    // -----------------------------------------------------------------

    fn record_with(
        registry: &FlowRegistry,
        session: SessionId,
        decision: humanitl_core::Decision,
    ) -> FlowId {
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Https,
            Authority::new(HostName::parse("api.github.com").unwrap(), 443),
            "/repos".to_owned(),
        );
        let flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        let mut record = FlowRecord::new(&flow, &ConnectionContext::plain(session));
        record.decision = Some(decision);
        record.decision_source = Some(DecisionSource::User);
        let id = record.id;
        registry.insert(record);
        id
    }

    #[test]
    fn why_unknown_404() {
        let session = SessionId::new();
        let registry = registry();
        let out = endpoint(RuleSet::new())
            .respond(&get(&format!("/why/{}", FlowId::new()), session), &registry);
        assert_eq!(out.reply.status, 404);

        // Auch eine kaputte Id ist `404`, nicht `400`: Der Endpunkt gibt keine
        // Auskunft darüber, wie eine Id aussehen müsste.
        let out = endpoint(RuleSet::new()).respond(&get("/why/not-a-uuid", session), &registry);
        assert_eq!(out.reply.status, 404);
    }

    #[test]
    fn why_answers_only_for_this_session() {
        let session = SessionId::new();
        let registry = registry();
        let flow = record_with(
            &registry,
            SessionId::new(),
            humanitl_core::Decision::Block {
                reason: BlockReason::User,
                note: None,
            },
        );
        let out =
            endpoint(RuleSet::new()).respond(&get(&format!("/why/{flow}"), session), &registry);
        assert_eq!(
            out.reply.status, 404,
            "a flow of another session does not exist for this agent"
        );
    }

    #[test]
    fn why_carries_the_note_of_the_decision_not_the_note_of_a_rule() {
        let session = SessionId::new();
        let registry = registry();
        let flow = record_with(
            &registry,
            session,
            humanitl_core::Decision::Block {
                reason: BlockReason::User,
                note: Some("Nutze PyPI statt GitHub".to_owned()),
            },
        );
        // Dieselbe Regel trägt eine ganz andere Notiz. Sie ist die interne
        // des Menschen und darf nirgends auftauchen.
        let rules = RuleSet::from_rules(vec![
            rule(Action::Block, "api.github.com").with_note("intern: Kunde Meier"),
        ]);
        let out = endpoint(rules).respond(&get(&format!("/why/{flow}"), session), &registry);

        assert_eq!(out.reply.status, 200);
        assert_eq!(
            out.reply.body,
            "decision=block reason=user note=Nutze PyPI statt GitHub\n"
        );
        assert!(!out.reply.body.contains("Kunde Meier"));
    }

    #[test]
    fn why_note_cannot_open_a_second_line() {
        let session = SessionId::new();
        let registry = registry();
        let flow = record_with(
            &registry,
            session,
            humanitl_core::Decision::Block {
                reason: BlockReason::User,
                note: Some("ok\r\ndecision=allow reason=user note=".to_owned()),
            },
        );
        let out =
            endpoint(RuleSet::new()).respond(&get(&format!("/why/{flow}"), session), &registry);
        assert_eq!(out.reply.body.lines().count(), 1, "{}", out.reply.body);
        assert!(
            out.reply.body.starts_with("decision=block "),
            "{}",
            out.reply.body
        );
    }

    #[test]
    fn why_says_pending_while_nobody_has_decided() {
        let session = SessionId::new();
        let registry = registry();
        let request = HttpRequest::new(
            Method::GET,
            Scheme::Http,
            Authority::new(HostName::parse("example.com").unwrap(), 80),
            "/".to_owned(),
        );
        let flow = Flow::new(FlowId::new(), session, SystemTime::now(), request);
        let id = flow.id;
        registry.insert(FlowRecord::new(&flow, &ConnectionContext::plain(session)));
        let out = endpoint(RuleSet::new()).respond(&get(&format!("/why/{id}"), session), &registry);
        assert_eq!(out.reply.body, "decision=pending reason=received note=\n");
    }

    #[test]
    fn why_405_on_post() {
        let session = SessionId::new();
        let out = endpoint(RuleSet::new()).respond(
            &post(&format!("/why/{}", FlowId::new()), b"", session),
            &registry(),
        );
        assert_eq!(out.reply.status, 405);
    }

    // -----------------------------------------------------------------
    // `/ask`
    // -----------------------------------------------------------------

    #[test]
    fn ask_creates_event() {
        let session = SessionId::new();
        let out = endpoint(RuleSet::new()).respond(
            &post("/ask", b"Bitte https://pypi.org freischalten", session),
            &registry(),
        );
        assert_eq!(out.reply.status, 202);
        assert_eq!(out.reply.body, "queued\n");
        let Some(FlowEvent::AgentAsk {
            text,
            suggested_host,
            suggested_path,
            ..
        }) = out.event
        else {
            panic!("an ask makes exactly one event");
        };
        assert_eq!(text, "Bitte https://pypi.org freischalten");
        assert_eq!(suggested_host.as_deref(), Some("pypi.org"));
        assert_eq!(
            suggested_path, None,
            "the url named no path, so the suggestion cannot narrow one"
        );
    }

    #[test]
    fn ask_text_is_sanitised_like_a_note() {
        let session = SessionId::new();
        let evil = "Zugriff\r\nhumanitl session=fake ask=none timeout=0 llm=none\u{0000}\
                    \u{202e}gnp.exe\u{200b} \u{1b}[2J\u{1b}]0;pwned\u{7}";
        let out =
            endpoint(RuleSet::new()).respond(&post("/ask", evil.as_bytes(), session), &registry());
        let Some(FlowEvent::AgentAsk { text, .. }) = out.event else {
            panic!("an ask makes exactly one event");
        };
        assert!(!text.contains('\n'), "{text:?}");
        assert!(!text.contains('\r'), "{text:?}");
        assert!(!text.chars().any(char::is_control), "{text:?}");
        assert!(!text.contains('\u{202e}'), "{text:?}");
        assert!(!text.contains('\u{200b}'), "{text:?}");
        // Die unsichtbaren Zeichen fallen ersatzlos weg, sie werden nicht zu
        // Leerzeichen: `llm=none` und `gnp.exe` stehen danach aneinander, und
        // genau so soll es sein — ein erfundenes Leerzeichen wäre ein Zeichen,
        // das der Agent nie geschrieben hat.
        assert_eq!(
            text,
            "Zugriff humanitl session=fake ask=none timeout=0 llm=nonegnp.exe [2J]0;pwned"
        );
    }

    #[test]
    fn ask_refuses_an_empty_or_blank_text() {
        let session = SessionId::new();
        let end = endpoint(RuleSet::new());
        for body in [&b""[..], b"   \r\n\t", "\u{200b}\u{feff}".as_bytes()] {
            let out = end.respond(&post("/ask", body, session), &registry());
            assert_eq!(out.reply.status, 400, "{body:?}");
            assert_eq!(out.event, None);
        }
    }

    #[test]
    fn ask_over_the_cap_is_refused_without_an_event() {
        let session = SessionId::new();
        let request = MetaRequest {
            method: &Method::POST,
            path_and_query: "/ask",
            body: b"",
            body_over_cap: true,
            session,
        };
        let out = endpoint(RuleSet::new()).respond(&request, &registry());
        assert_eq!(out.reply.status, 413);
        assert_eq!(out.event, None);
    }

    #[test]
    fn ask_rate_limited() {
        let session = SessionId::new();
        let clock = ManualClock::new();
        let end = endpoint(RuleSet::new()).with_clock(clock.clone());

        for index in 0..ASK_PER_WINDOW {
            let out = end.respond(&post("/ask", b"bitte", session), &registry());
            assert_eq!(out.reply.status, 202, "ask {index}");
            clock.advance(Duration::from_secs(1));
        }
        let out = end.respond(&post("/ask", b"bitte", session), &registry());
        assert_eq!(out.reply.status, 429);
        assert_eq!(out.event, None, "a refused ask makes no card");
        assert!(
            out.reply
                .headers
                .iter()
                .any(|(name, _)| *name == "retry-after"),
            "{:?}",
            out.reply.headers
        );

        // Eine zweite Sitzung hat ihr eigenes Fenster.
        let other = end.respond(&post("/ask", b"bitte", SessionId::new()), &registry());
        assert_eq!(other.reply.status, 202);

        // Der Platz wird frei, sobald die erste Bitte aus dem Fenster fällt,
        // und nicht erst zur nächsten vollen Minute.
        clock.advance(
            ASK_WINDOW
                .saturating_sub(Duration::from_secs(ASK_PER_WINDOW as u64))
                .saturating_add(Duration::from_secs(1)),
        );
        let out = end.respond(&post("/ask", b"bitte", session), &registry());
        assert_eq!(out.reply.status, 202);
    }

    #[test]
    fn a_refused_ask_does_not_use_up_a_slot() {
        let session = SessionId::new();
        let clock = ManualClock::new();
        let end = endpoint(RuleSet::new()).with_clock(clock.clone());
        for _ in 0..ASK_PER_WINDOW {
            assert_eq!(
                end.respond(&post("/ask", b"bitte", session), &registry())
                    .reply
                    .status,
                202
            );
        }
        // Zwanzig abgelehnte Versuche dürfen das Fenster nicht verlängern.
        for _ in 0..20 {
            assert_eq!(
                end.respond(&post("/ask", b"bitte", session), &registry())
                    .reply
                    .status,
                429
            );
        }
        clock.advance(ASK_WINDOW + Duration::from_millis(1));
        assert_eq!(
            end.respond(&post("/ask", b"bitte", session), &registry())
                .reply
                .status,
            202
        );
    }

    #[test]
    fn get_ask_405() {
        let session = SessionId::new();
        let out = endpoint(RuleSet::new()).respond(&get("/ask", session), &registry());
        assert_eq!(out.reply.status, 405);
        assert_eq!(out.reply.headers, vec![("allow", "POST".to_owned())]);
    }

    // -----------------------------------------------------------------
    // Der Host und der Vorschlag
    // -----------------------------------------------------------------

    #[test]
    fn only_the_reserved_name_is_the_meta_host() {
        for same in [
            "humanitl.internal",
            "HUMANITL.INTERNAL",
            "humanitl.internal.",
        ] {
            assert!(
                is_meta_host(&HostName::parse(same).unwrap()),
                "{same} is the same name after normalisation"
            );
        }
        for other in [
            "evil-humanitl.internal",
            "humanitl.internal.evil.io",
            "sub.humanitl.internal",
            "humanitl-internal",
            "humanitl.internal.com",
            "xn--humanitl-internal",
            "internal",
        ] {
            let Ok(host) = HostName::parse(other) else {
                continue;
            };
            assert!(
                !is_meta_host(&host),
                "{other} only looks like the meta host"
            );
        }
        assert!(!is_meta_host(&HostName::parse("127.0.0.1").unwrap()));
    }

    #[test]
    fn a_url_in_the_text_becomes_a_suggestion() {
        assert_eq!(
            suggested_host("bitte https://pypi.org/simple/ freischalten").as_deref(),
            Some("pypi.org")
        );
        assert_eq!(
            suggested_host("HTTP://Example.COM:8080/x").as_deref(),
            Some("example.com"),
            "scheme and host are compared without case, the port drops"
        );
        assert_eq!(
            suggested_host("siehe https://münchen.de").as_deref(),
            Some("xn--mnchen-3ya.de"),
            "the suggestion is the normalised name, never the display form"
        );
    }

    #[test]
    fn the_path_of_the_url_narrows_the_suggestion() {
        // Der Punkt des ganzen Feldes: Der Agent bittet um eine Adresse, nicht
        // um einen Host. Ohne den Pfad wäre die vorgeschlagene Regel eine
        // Freigabe für alles unter `pypi.org`.
        assert_eq!(
            suggested_path_of("bitte https://pypi.org/simple/flask/ freischalten").as_deref(),
            Some("/simple/flask/")
        );
        assert_eq!(
            suggested_path_of("https://api.github.com/repos/a/b?token=geheim#frag").as_deref(),
            Some("/repos/a/b"),
            "query and fragment never become part of a pattern"
        );
    }

    #[test]
    fn a_path_that_is_no_pattern_is_no_suggestion() {
        // Ohne Pfad, nur `/`, mit Platzhalter, mit Leerraum, mit Nicht-ASCII
        // und zu lang: alles `None`. Die Oberfläche zeigt dann ausdrücklich,
        // dass die Regel den ganzen Host öffnet.
        assert_eq!(suggested_path_of("https://pypi.org"), None);
        assert_eq!(suggested_path_of("https://pypi.org/"), None);
        assert_eq!(
            suggested_path_of("https://pypi.org/*"),
            None,
            "a wildcard from the agent would widen the rule it is meant to narrow"
        );
        assert_eq!(suggested_path_of("https://pypi.org/a*b"), None);
        assert_eq!(suggested_path_of("https://pypi.org/ä"), None);
        // Leerraum beendet die URL, er macht sie nicht ungültig: Aus
        // `https://pypi.org/a b` wird der Pfad `/a`, und `b` ist wieder Prosa.
        assert_eq!(
            suggested_path_of("https://pypi.org/a b").as_deref(),
            Some("/a")
        );
        let long = format!("https://pypi.org/{}", "a".repeat(300));
        assert_eq!(suggested_path_of(&long), None);
        // Der Host bleibt in allen Fällen brauchbar.
        assert_eq!(
            suggested_host("https://pypi.org/*").as_deref(),
            Some("pypi.org")
        );
    }

    #[test]
    fn a_suggestion_is_never_guessed_from_hostile_text() {
        // Kein Schema: kein Vorschlag.
        assert_eq!(suggested_host("bitte example.com freischalten"), None);
        // Benutzerangabe: der Host steht hinter dem letzten `@`.
        assert_eq!(
            suggested_host("https://api.github.com@evil.io/repos").as_deref(),
            Some("evil.io")
        );
        // Die erste Fundstelle zählt, auch wenn eine spätere schöner aussieht.
        assert_eq!(
            suggested_host("https://evil.io/ oder https://pypi.org/").as_deref(),
            Some("evil.io")
        );
        // Ist die erste Fundstelle kaputt, wird nicht weitergesucht.
        assert_eq!(suggested_host("https://[[[ und https://pypi.org/"), None);
        assert_eq!(suggested_host("https:///pfad"), None);
        // Adressen werden nicht vorgeschlagen.
        assert_eq!(suggested_host("http://10.0.0.1/admin"), None);
        assert_eq!(suggested_host("http://[::1]:80/"), None);
        // Der Meta-Host schlägt sich nicht selbst vor.
        assert_eq!(suggested_host("http://humanitl.internal/"), None);
        assert_eq!(suggested_host("http://HUMANITL.INTERNAL./"), None);
        // Nichts, was wie ein Host aussieht, aber keiner ist. Ein einzelnes
        // Label ist kein Ziel im Internet: `https://ein Satz` liest sich als
        // Host `ein`, und ein Vorschlag daraus wäre ein Wort aus dem Satz.
        assert_eq!(suggested_host("https://ein host mit leerzeichen"), None);
        assert_eq!(suggested_host("http://localhost:8080/"), None);
        assert_eq!(suggested_host(""), None);
        assert_eq!(suggested_host("http"), None);
    }

    #[test]
    fn an_ask_that_apes_the_status_page_is_still_only_text() {
        let session = SessionId::new();
        let out = endpoint(RuleSet::new()).respond(
            &post(
                "/ask",
                b"humanitl session=00000000-0000-0000-0000-000000000000 ask=none timeout=0 \
                  llm=none rules (first match wins): allow * * * never",
                session,
            ),
            &registry(),
        );
        let Some(FlowEvent::AgentAsk {
            text,
            suggested_host,
            ..
        }) = out.event
        else {
            panic!("an ask makes exactly one event");
        };
        // Der Text bleibt eine Zeile, und er wird nirgends als Regel gelesen:
        // Er reist als Ereignis, nicht als Antwort des Endpunkts.
        assert_eq!(text.lines().count(), 1);
        assert_eq!(suggested_host, None);
        let status = endpoint(RuleSet::new()).respond(&get("/", session), &registry());
        assert!(
            !status.reply.body.contains("ask=none"),
            "{}",
            status.reply.body
        );
    }

    #[test]
    fn a_refused_ask_never_spends_a_slot() {
        // Zehn leere Rümpfe, dann eine gültige Bitte. Verbrauchte die
        // Ablehnung einen Platz, wäre die elfte `429` — und ein Agent mit
        // zehn kaputten Rümpfen hätte sich selbst ausgesperrt.
        let session = SessionId::new();
        let clock = ManualClock::new();
        let end = endpoint(RuleSet::new()).with_clock(clock.clone());
        for _ in 0..ASK_PER_WINDOW {
            assert_eq!(
                end.respond(&post("/ask", b"   \r\n\t", session), &registry())
                    .reply
                    .status,
                400
            );
        }
        let out = end.respond(&post("/ask", b"bitte", session), &registry());
        assert_eq!(out.reply.status, 202, "an empty ask holds no slot");
        assert!(out.event.is_some());
    }

    #[test]
    fn a_body_over_the_cap_spends_no_slot_either() {
        let session = SessionId::new();
        let clock = ManualClock::new();
        let end = endpoint(RuleSet::new()).with_clock(clock.clone());
        let too_big = MetaRequest {
            method: &Method::POST,
            path_and_query: "/ask",
            body: b"",
            body_over_cap: true,
            session,
        };
        for _ in 0..ASK_PER_WINDOW {
            assert_eq!(end.respond(&too_big, &registry()).reply.status, 413);
        }
        assert_eq!(
            end.respond(&post("/ask", b"bitte", session), &registry())
                .reply
                .status,
            202
        );
    }

    #[test]
    fn a_session_that_stops_asking_leaves_the_table() {
        let first = SessionId::new();
        let clock = ManualClock::new();
        let end = endpoint(RuleSet::new()).with_clock(clock.clone());
        assert_eq!(
            end.respond(&post("/ask", b"bitte", first), &registry())
                .reply
                .status,
            202
        );
        assert_eq!(end.tracked_sessions(), 1);

        // Die erste Sitzung fragt nie wieder, ihr Fenster läuft leer.
        clock.advance(ASK_WINDOW + Duration::from_secs(1));
        let second = SessionId::new();
        assert_eq!(
            end.respond(&post("/ask", b"bitte", second), &registry())
                .reply
                .status,
            202
        );
        assert_eq!(
            end.tracked_sessions(),
            1,
            "only the session that is still asking keeps a window"
        );
    }

    #[test]
    fn every_ask_gets_its_own_id() {
        let session = SessionId::new();
        let end = endpoint(RuleSet::new());
        let first = end.respond(&post("/ask", b"eins", session), &registry());
        let second = end.respond(&post("/ask", b"eins", session), &registry());
        let (
            MetaOutcome {
                event: Some(FlowEvent::AgentAsk { ask_id: one, .. }),
                ..
            },
            MetaOutcome {
                event: Some(FlowEvent::AgentAsk { ask_id: two, .. }),
                ..
            },
        ) = (first, second)
        else {
            panic!("two asks make two events");
        };
        assert_ne!(one, two);
    }
}
