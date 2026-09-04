//! Gescheiterte TLS-Handschläge des Clients, gedeutet (HUM-045).
//!
//! Nicht jedes Werkzeug in der Sandbox vertraut der Humanitl-CA: die JVM liest
//! keine der CA-Umgebungsvariablen, ältere Bun-Versionen kennen
//! `NODE_EXTRA_CA_CERTS` nicht, ein Go-Programm mit eigenem Zertifikatsspeicher
//! ignoriert `SSL_CERT_FILE`. Das Symptom ist ein TLS-Fehler im Terminal des
//! Agenten, und ohne diese Datei sieht ihn nur der Agent. Der Proxy sitzt am
//! anderen Ende desselben Handschlags; er sieht den Abbruch, kann ihn benennen
//! und einen Vorschlag machen.
//!
//! # Was hier entschieden wird und was nicht
//!
//! [`classify`] deutet den Fehler des Handschlags, [`tool_hint`] rät aus dem
//! `User-Agent` des `CONNECT`, welches Werkzeug dahintersteckt, und
//! [`diagnostic_for`] baut daraus den Befund. Ob er auch veröffentlicht wird,
//! entscheidet die [`HandshakeWatch`]: ein einzelner Abbruch ohne Alert ist ein
//! Alltagsereignis (ein Client, der es sich anders überlegt), erst drei in zehn
//! Sekunden zum selben Host sind ein Muster. Und derselbe Befund wird für
//! dieselbe Paarung eine Minute lang nur einmal gemeldet, damit ein Werkzeug in
//! einer Wiederholschleife nicht den Ereignisstrom füllt. Was dabei unterdrückt
//! wird, nennt der nächste Befund für denselben Host und denselben Hinweis —
//! solange dessen Eintrag lebt. Wird er verdrängt (siehe unten), beginnt der
//! Zähler wieder bei eins; verloren ist damit nur die Zahl, nicht der Vorgang:
//! Jeder einzelne Versuch steht als eigene Zeile in der History.
//!
//! # Der Hinweis ist ein Hinweis
//!
//! Der `User-Agent` kommt aus der Sandbox und ist damit vom beobachteten
//! Prozess selbst geschrieben. Er wird nie wörtlich in einen Befundtext
//! übernommen, sondern nur auf eine der bekannten Ausprägungen von
//! [`ToolHint`] abgebildet; der Text sagt deshalb „ein Client, der sich curl
//! nennt", nicht „curl" (`backlog/CONVENTIONS.md` 4.13: nie mehr behaupten als
//! belegt ist). Fehlt der Kopf oder passt er auf nichts, ist
//! [`ToolHint::Unknown`] der häufige und richtige Fall.
//!
//! # Speicher und Ausgabe
//!
//! Ein Client, der den Handschlag endlos abbricht, darf weder im Zählfenster
//! unbegrenzt Platz belegen noch unbegrenzt Karten erzeugen. Beides ist
//! gedeckelt, und zwar getrennt:
//!
//! - **Speicher.** Je Host bleiben höchstens [`DROP_THRESHOLD`] Zeitpunkte
//!   stehen, und es werden höchstens [`MAX_TRACKED_HOSTS`] Hosts verfolgt;
//!   darüber hinaus fällt zuerst weg, was aus jedem Fenster gelaufen ist
//!   (dort ist auch die Entdopplung abgelaufen, es geht also nichts verloren),
//!   und erst danach der am längsten nicht gesehene Host.
//! - **Ausgabe.** Die Verdrängung eines noch lebenden Eintrags nimmt dessen
//!   Entdopplung mit; ein Client, der über mehr als [`MAX_TRACKED_HOSTS`] Namen
//!   rotiert, bekäme sonst für jeden Versuch wieder eine Karte. Deshalb liegt
//!   über allen Hosts zusammen ein zweiter Deckel:
//!   [`MAX_REPORTS_PER_WINDOW`] Befunde je [`REPEAT_WINDOW`].
//!
//! Der zweite Deckel zählt **getrennt nach Schweregrad**, und das ist keine
//! Feinheit, sondern der Punkt: Der Client in der Sandbox wählt Zielnamen und
//! SNI selbst und braucht dafür kein DNS. Mit einem gemeinsamen Topf könnte er
//! ihn mit [`TLS_003`] (Info, ein Handschlag ohne SNI zu einem Fantasienamen)
//! leerlaufen lassen und damit genau die Warnung unterdrücken, für die dieses
//! Modul existiert: dass ein Mensch von einem abgelehnten Zertifikat erfährt.
//! Eine Information darf eine Warnung nicht verhungern lassen. `TLS_001` und
//! `TLS_002` zahlen deshalb aus dem Topf der Warnungen, `TLS_003` aus dem der
//! Informationen, und keiner der beiden kann den anderen leeren.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::error::Error;
use std::io;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::{TLS_001, TLS_002, TLS_003};
use humanitl_core::{
    Action, Diagnostic, FixAction, HeaderMap, HostName, HostPattern, Matcher, Rule, RuleId,
    Severity,
};

use crate::ca::SANDBOX_CA_PATH;

/// So lange zählt ein abgebrochener Handschlag mit (`TLS_002`).
pub const DROP_WINDOW: Duration = Duration::from_secs(10);

/// So viele Abbrüche in [`DROP_WINDOW`] ergeben ein Muster (`TLS_002`).
pub const DROP_THRESHOLD: usize = 3;

/// So lange wird derselbe Befund für dieselbe Paarung nicht wiederholt.
pub const REPEAT_WINDOW: Duration = Duration::from_secs(60);

/// So viele Hosts hält das Zählfenster höchstens.
pub const MAX_TRACKED_HOSTS: usize = 256;

/// So viele Befunde gehen höchstens je [`REPEAT_WINDOW`] und je Schweregrad in
/// den Ereignisstrom, über alle Hosts zusammen.
///
/// Der Deckel über der Entdopplung je Host: Er hält auch dann, wenn ein Client
/// über mehr Namen rotiert, als das Zählfenster verfolgt. Gezählt wird getrennt
/// nach Warnung (`TLS_001`, `TLS_002`) und Information (`TLS_003`), damit das
/// eine das andere nicht verdrängen kann (siehe Modulkommentar). Die History
/// bleibt davon unberührt — jeder Versuch ist dort eine eigene Zeile.
pub const MAX_REPORTS_PER_WINDOW: usize = 32;

/// Der Wert, der bei einem gescheiterten Handschlag in `flows.error` steht.
pub const FLOW_ERROR: &str = "tls_handshake_failed";

/// So tief wird die Ursachenkette eines Fehlers verfolgt.
const MAX_CAUSE_DEPTH: usize = 8;

/// Warum der Handschlag mit dem Client nicht zustande kam.
///
/// Nur die Fälle, die etwas über den Client aussagen. Was sich nicht einordnen
/// lässt, ist kein Befund: [`classify`] liefert dann `None`, und der Proxy
/// schreibt nur eine Zeile ins Protokoll. Eine falsche Erklärung wäre
/// schlimmer als keine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TlsFailure {
    /// Der Client hat mit `unknown_ca` abgelehnt: Er kennt die Humanitl-CA nicht.
    AlertUnknownCa,
    /// Der Client hat mit `bad_certificate` abgelehnt.
    AlertBadCertificate,
    /// Ein anderer Alert; die Beschreibung, wie rustls sie nennt.
    AlertOther(String),
    /// Der Client hat die Verbindung geschlossen, bevor der Handschlag stand.
    ///
    /// Der Proxy las ein Dateiende, während rustls noch auf die nächste
    /// Nachricht des Clients wartete.
    EofBeforeFinished,
    /// Die Verbindung brach ab, bevor der Handschlag stand.
    ///
    /// Zurückgesetzt, abgebrochen oder — der häufige Fall bei einem Client, der
    /// nach dem `ClientHello` einfach auflegt — beim Schreiben der Antwort
    /// abgerissen (`EPIPE`). In `tls::accept` gibt es genau eine Gegenstelle,
    /// den Client; ein gebrochener Schreibweg dorthin heißt, dass er weg ist.
    ResetBeforeFinished,
}

impl TlsFailure {
    /// Kurzname in `snake_case`, für Protokollzeilen.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::AlertUnknownCa => "alert_unknown_ca",
            Self::AlertBadCertificate => "alert_bad_certificate",
            Self::AlertOther(_) => "alert_other",
            Self::EofBeforeFinished => "eof_before_finished",
            Self::ResetBeforeFinished => "reset_before_finished",
        }
    }

    /// Wahr, wenn der Client die Ablehnung des Zertifikats ausgesprochen hat.
    ///
    /// Nur diese beiden Alerts belegen, dass der Client das Leaf gesehen und
    /// verworfen hat; nur sie führen zu [`TLS_001`].
    #[must_use]
    pub const fn is_rejection(&self) -> bool {
        matches!(self, Self::AlertUnknownCa | Self::AlertBadCertificate)
    }

    /// Wahr, wenn der Handschlag abgebrochen wurde, ohne dass ein Alert die
    /// CA nennt.
    ///
    /// Ein einzelnes Vorkommen sagt nichts; gezählt wird es trotzdem, weil
    /// drei davon in zehn Sekunden [`TLS_002`] ergeben.
    #[must_use]
    pub const fn is_drop(&self) -> bool {
        !self.is_rejection()
    }
}

/// Welches Werkzeug hinter dem `CONNECT` vermutet wird.
///
/// Ein Hinweis, keine Tatsache: Der `User-Agent` kommt aus der Sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolHint {
    /// `curl/8.5.0`
    Curl,
    /// `node`, `undici`, `node-fetch`
    Node,
    /// `Bun/1.1.0`
    Bun,
    /// `python-requests/2.31.0`, `Python-urllib/3.12`
    Python,
    /// `Go-http-client/1.1`
    Go,
    /// `Java/21`, `okhttp/4.12.0`
    Java,
    /// `git/2.43.0`
    Git,
    /// `cargo/1.88.0`
    Cargo,
    /// Kein `User-Agent`, oder keiner, der auf eines der bekannten Werkzeuge
    /// passt. Der häufige Fall.
    Unknown,
}

impl ToolHint {
    /// Kurzname in `snake_case`, für Protokollzeilen und die Entdopplung.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Curl => "curl",
            Self::Node => "node",
            Self::Bun => "bun",
            Self::Python => "python",
            Self::Go => "go",
            Self::Java => "java",
            Self::Git => "git",
            Self::Cargo => "cargo",
            Self::Unknown => "unknown",
        }
    }

    /// Das Subjekt des Befundtextes.
    ///
    /// Nennt den Hinweis als Hinweis. Der Text behauptet nicht, dass dort curl
    /// lief, sondern dass sich etwas so genannt hat.
    #[must_use]
    pub fn subject(self) -> String {
        match self {
            Self::Unknown => "A client".to_owned(),
            other => format!("A client that calls itself {}", other.as_str()),
        }
    }

    /// Die Umgebungsvariable, die dieses Werkzeug auf die CA lenkt.
    ///
    /// `None` für die JVM: Sie liest keine dieser Variablen, und Humanitl legt
    /// noch keinen Java-Truststore an (siehe [`crate::ca`], HUM-014). Eine
    /// Variable vorzuschlagen, die auf eine Datei zeigt, die es nicht gibt,
    /// machte den Fehler schlimmer statt besser.
    ///
    /// Genau eine Variable, nie zwei: [`FixAction::SetEnv`] trägt ein Paar.
    /// Python bräuchte `SSL_CERT_FILE` **und** `REQUESTS_CA_BUNDLE`; der Knopf
    /// setzt `SSL_CERT_FILE`, und [`ToolHint::note`] sagt im Befundtext, dass
    /// die zweite von Hand dazugehört. Ein Feld für mehrere Vorschläge gehört
    /// in [`Diagnostic`] und damit in `humanitl-core`, nicht hierher.
    #[must_use]
    pub fn fix(self) -> Option<FixAction> {
        let key = self.ca_variable()?;
        Some(FixAction::SetEnv {
            key: key.to_owned(),
            value: SANDBOX_CA_PATH.to_owned(),
        })
    }

    /// Der Name der Variablen aus [`ToolHint::fix`], ohne den Vorschlag.
    ///
    /// Jede davon steht im Env-Kit ([`crate::ca::ENV_KIT`]) und ist in der
    /// Sandbox also schon gesetzt; der Befundtext nennt sie deshalb als
    /// Tatsache und nicht als Vermutung.
    #[must_use]
    pub const fn ca_variable(self) -> Option<&'static str> {
        match self {
            Self::Curl => Some("CURL_CA_BUNDLE"),
            Self::Node | Self::Bun => Some("NODE_EXTRA_CA_CERTS"),
            Self::Git => Some("GIT_SSL_CAINFO"),
            Self::Cargo => Some("CARGO_HTTP_CAINFO"),
            Self::Python | Self::Go | Self::Unknown => Some("SSL_CERT_FILE"),
            Self::Java => None,
        }
    }

    /// Der Satz, der zu diesem Werkzeug noch gesagt werden muss.
    ///
    /// Leer für die Werkzeuge, bei denen die Variable allein genügt.
    #[must_use]
    pub const fn note(self) -> &'static str {
        match self {
            Self::Curl => {
                " curl's --cacert and --capath override CURL_CA_BUNDLE, and --insecure skips the \
                 check altogether."
            }
            Self::Bun => {
                " Bun reads NODE_EXTRA_CA_CERTS from version 1.1.22 on; older versions ignore it."
            }
            Self::Python => {
                " Python reads SSL_CERT_FILE and the requests library reads REQUESTS_CA_BUNDLE; \
                 Humanitl sets both in the sandbox, and the fix below carries only the first \
                 because one fix action holds one pair."
            }
            Self::Go => {
                " A Go program that builds its own tls.Config with a RootCAs pool never looks at \
                 SSL_CERT_FILE."
            }
            Self::Java => {
                " The JVM reads none of the CA environment variables, and Humanitl does not build \
                 a Java truststore yet, so no variable in the sandbox points a JVM at Humanitl's \
                 CA."
            }
            Self::Unknown => {
                " The CONNECT named no tool Humanitl knows, so SSL_CERT_FILE is the variable that \
                 fits most of them."
            }
            _ => "",
        }
    }
}

/// Rät aus dem `User-Agent` des `CONNECT`, welches Werkzeug verbunden ist.
///
/// Verglichen wird der Produktname eines Tokens, nicht ein Teilstring: `git/2.4`
/// ist Git, `GitHub-Hookshot/1.0` ist es nicht. Fehlt der Kopf, ist er leer oder
/// passt er auf keinen bekannten Namen, ist das Ergebnis [`ToolHint::Unknown`].
#[must_use]
pub fn tool_hint(connect_headers: &HeaderMap) -> ToolHint {
    let Some(agent) = connect_headers
        .get(http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
    else {
        return ToolHint::Unknown;
    };
    agent
        .split_whitespace()
        .find_map(|token| {
            let product = token.split('/').next().unwrap_or(token);
            hint_for_product(&product.to_ascii_lowercase())
        })
        .unwrap_or(ToolHint::Unknown)
}

/// Der Hinweis zu einem Produktnamen aus dem `User-Agent`, klein geschrieben.
fn hint_for_product(product: &str) -> Option<ToolHint> {
    match product {
        "curl" => Some(ToolHint::Curl),
        "bun" => Some(ToolHint::Bun),
        "node" | "node-fetch" | "undici" | "axios" => Some(ToolHint::Node),
        "python" | "python-requests" | "python-urllib" | "python-httpx" | "aiohttp" | "httpx" => {
            Some(ToolHint::Python)
        }
        "go" | "go-http-client" => Some(ToolHint::Go),
        "java" | "java-http-client" | "okhttp" => Some(ToolHint::Java),
        "git" | "git-lfs" => Some(ToolHint::Git),
        "cargo" => Some(ToolHint::Cargo),
        _ => None,
    }
}

/// Deutet den Fehler eines Handschlags, falls er sich deuten lässt.
///
/// Die Ursachenkette wird bis zu acht Stufen tief verfolgt, weil
/// `tokio-rustls` einen `rustls::Error` in einen [`io::Error`] mit
/// [`io::ErrorKind::InvalidData`] legt. Der innere Fehler ist über
/// [`io::Error::get_ref`] erreichbar und nicht über
/// [`Error::source`]: `io::Error::source` gibt die Ursache *seiner* Ursache
/// zurück, überspringt also genau den Wert, um den es hier geht.
///
/// `None` heißt: Der Fehler sagt nichts über den Client aus (ein
/// Protokollfehler, ein abgebrochener Task, ein Schreibfehler). Dann wird kein
/// Befund erfunden.
///
/// Die Signatur der Spezifikation lautet `&dyn std::error::Error`; hier steht
/// `&(dyn Error + 'static)`, weil `downcast_ref` die Lebensdauer braucht. Der
/// Aufrufer merkt davon nichts: Ein `&io::Error` passt unverändert.
#[must_use]
pub fn classify(err: &(dyn Error + 'static)) -> Option<TlsFailure> {
    let mut current = Some(err);
    for _ in 0..MAX_CAUSE_DEPTH {
        let node = current?;
        if let Some(tls) = node.downcast_ref::<rustls::Error>() {
            return from_alert(tls);
        }
        if let Some(error) = node.downcast_ref::<io::Error>() {
            if let Some(failure) = from_io_kind(error.kind()) {
                return Some(failure);
            }
            current = error.get_ref().map(|inner| inner as &(dyn Error + 'static));
            continue;
        }
        current = node.source();
    }
    None
}

/// Der Alert, den der Client geschickt hat, falls es einer war.
fn from_alert(err: &rustls::Error) -> Option<TlsFailure> {
    let rustls::Error::AlertReceived(alert) = err else {
        return None;
    };
    match *alert {
        rustls::AlertDescription::UnknownCA => Some(TlsFailure::AlertUnknownCa),
        rustls::AlertDescription::BadCertificate => Some(TlsFailure::AlertBadCertificate),
        other => Some(TlsFailure::AlertOther(format!("{other:?}"))),
    }
}

/// Der Abbruch, den die Art eines [`io::Error`] beschreibt.
///
/// Nur die Arten, die belegen, dass der Client weg ist. `InvalidData` trägt den
/// `rustls::Error` und wird eine Stufe tiefer gedeutet; alles andere sagt
/// nichts über den Client und bleibt ungedeutet.
const fn from_io_kind(kind: io::ErrorKind) -> Option<TlsFailure> {
    match kind {
        io::ErrorKind::UnexpectedEof => Some(TlsFailure::EofBeforeFinished),
        io::ErrorKind::ConnectionReset
        | io::ErrorKind::ConnectionAborted
        | io::ErrorKind::BrokenPipe => Some(TlsFailure::ResetBeforeFinished),
        _ => None,
    }
}

/// Der Befund zu einem gescheiterten Handschlag.
///
/// Eine ausgesprochene Ablehnung ([`TlsFailure::is_rejection`]) ergibt
/// [`TLS_001`] samt dem Vorschlag, der zum Hinweis passt; ein Abbruch ohne
/// Alert ergibt [`TLS_002`]. Der zweite Fall gehört nur dann in den
/// Ereignisstrom, wenn die [`HandshakeWatch`] ein Muster gesehen hat — der Text
/// spricht deshalb von „keeps dropping".
///
/// `since_last` ist die Zahl der Versuche, die seit der letzten Karte für diese
/// Paarung gezählt wurden, den jetzigen eingeschlossen; sie kommt aus
/// [`HandshakeWatch::on_rejection`] und steht ab zwei im Text. Die
/// Spezifikation nennt für diese Funktion drei Parameter; der vierte ist der
/// Zähler, den ihr Fallstrick verlangt („dedupe pro (Host, Hint) für 60 s,
/// Zähler in der Karte").
#[must_use]
pub fn diagnostic_for(
    failure: &TlsFailure,
    host: &HostName,
    hint: ToolHint,
    since_last: u32,
) -> Diagnostic {
    if failure.is_rejection() {
        return rejected_ca(host, hint, since_last);
    }
    repeated_drops(host)
}

/// [`TLS_001`]: Der Client hat das Leaf des Proxys abgelehnt.
///
/// Der Text behauptet keine fehlende Variable. Jede Variable, die
/// [`ToolHint::fix`] vorschlägt, steht schon im Env-Kit
/// ([`crate::ca::ENV_KIT`]) und ist in der Sandbox gesetzt, bevor das Werkzeug
/// startet; `profiles/sandbox/default.toml` trägt dieselben Paare, und
/// `daemon/crates/sandbox/tests/profile_parse.rs` gleicht beide ab. Belegt ist
/// deshalb nur, dass der Client das Leaf verworfen hat. Woran es liegen kann —
/// eine Option auf der Kommandozeile, ein eigener Zertifikatsspeicher, ein
/// angehefteter Schlüssel — nennt der Text als Möglichkeiten, und der Vorschlag
/// wird als das benannt, was er leistet: Er sorgt dafür, dass die Variable auch
/// dann gesetzt ist, wenn ein eigenes Profil sie nicht mitbringt.
fn rejected_ca(host: &HostName, hint: ToolHint, since_last: u32) -> Diagnostic {
    let path = SANDBOX_CA_PATH;
    let known = match hint.ca_variable() {
        Some(key) => format!(
            " Humanitl already sets {key}={path} in the sandbox, so this is not a missing \
             variable: the client overrode it on its command line, keeps its own certificate \
             pool, pins a certificate, or does not read {key} at all. Setting {key} under \
             [sandbox.env] in config.toml helps only where the sandbox profile in use does not \
             carry the variable."
        ),
        None => String::new(),
    };
    let why = format!(
        "{subject} inside the sandbox rejected Humanitl's certificate for {host}. The request \
         never left the sandbox.{known}{note}{repeats}",
        subject = hint.subject(),
        host = host.display(),
        note = hint.note(),
        repeats = repeat_sentence(since_last),
    );
    let builder = Diagnostic::builder(TLS_001, Severity::Warning).why(why);
    match hint.fix() {
        Some(fix) => builder.fix(fix).build(),
        None => builder.build(),
    }
}

/// Der Satz mit dem Zähler, oder nichts beim ersten Mal.
///
/// Genannt wird, was gezählt wurde, und worüber: Der Zähler läuft je Host
/// **und** je [`ToolHint`], weil auch die Entdopplung so läuft. Zwei Werkzeuge
/// am selben Host ergeben zwei Karten mit zwei Zählern; „to this host" allein
/// läse sich als Summe und wäre eine Zahl, die niemand gezählt hat. Keine Rate,
/// keine Schätzung.
fn repeat_sentence(since_last: u32) -> String {
    if since_last < 2 {
        return String::new();
    }
    format!(
        " {since_last} rejected handshakes have been counted for this host and this tool hint \
         since the previous card, this one included."
    )
}

/// [`TLS_002`]: Zum selben Host bricht der Handschlag wiederholt ab.
///
/// Der Vorschlag ist eine Block-Regel für diesen Host: Der Agent scheitert dann
/// sofort und mit einer Meldung, statt jedes Mal in denselben Abbruch zu
/// laufen. Angelegt wird sie nicht hier — ein [`FixAction`] ist ein Vorschlag,
/// den ein Mensch annimmt oder nicht.
#[must_use]
pub fn repeated_drops(host: &HostName) -> Diagnostic {
    let why = format!(
        "A client in the sandbox keeps dropping the TLS handshake to {host}. This usually means \
         certificate pinning or a tool that ignores CA variables.",
        host = host.display(),
    );
    let rule = Rule::new(
        RuleId::new(),
        Action::Block,
        Matcher::host(HostPattern::Exact(host.clone())),
    )
    .with_note(format!(
        "suggested after repeated TLS handshake failures to {host}",
        host = host.display(),
    ));
    Diagnostic::builder(TLS_002, Severity::Warning)
        .why(why)
        .fix(FixAction::AddRule(Box::new(rule)))
        .build()
}

/// [`TLS_003`]: Der Client hat im `ClientHello` keinen Namen genannt.
///
/// Die Spezifikation begründet diesen Code damit, dass sich ohne Namen kein
/// Zertifikat ausstellen ließe. Für diesen Proxy stimmt das nicht: Das Leaf
/// gilt dem Ziel des `CONNECT`, der Handschlag kommt also zustande. Was
/// wirklich geschieht, steht in
/// [`check_authority`](crate::connect::check_authority): Ohne SNI ist nicht
/// belegt, dass der Handschlag demselben Namen galt wie der Tunnel, und jede
/// Anfrage darin wird ohne Rückfrage abgelehnt. Der Text sagt das und nicht
/// mehr.
#[must_use]
pub fn missing_sni(host: &HostName) -> Diagnostic {
    Diagnostic::builder(TLS_003, Severity::Info)
        .why(format!(
            "A client in the sandbox opened a TLS connection to {host} without SNI. Nothing then \
             ties the handshake to {host}, so every request inside this connection is refused.",
            host = host.display(),
        ))
        .build()
}

/// Das Zählfenster: was zu welchem Host schon gemeldet wurde.
///
/// Eine je Handler, also je Sitzung. Alle Methoden sind synchron und kurz; die
/// Sperre wird nie über einen `await` gehalten.
#[derive(Debug, Default)]
pub struct HandshakeWatch {
    inner: Mutex<Watch>,
}

/// Der Inhalt des Fensters hinter der Sperre.
#[derive(Debug, Default)]
struct Watch {
    /// Je Host, was dort gezählt und gemeldet wurde.
    hosts: HashMap<String, HostState>,
    /// Der Deckel für `TLS_001` und `TLS_002`.
    warnings: Budget,
    /// Der Deckel für `TLS_003`, getrennt vom obigen: Eine Information darf
    /// eine Warnung nicht verhungern lassen (siehe Modulkommentar).
    infos: Budget,
}

/// Ein Deckel: die Zeitpunkte der zuletzt gemeldeten Befunde eines
/// Schweregrads.
///
/// Höchstens [`MAX_REPORTS_PER_WINDOW`] Einträge, also höchstens ein paar
/// hundert Byte, egal was der Client tut.
#[derive(Debug, Default)]
struct Budget {
    stamps: VecDeque<Instant>,
}

impl Budget {
    /// Wahr, wenn das Fenster noch einen Befund hergibt; räumt dabei auf.
    fn left(&mut self, now: Instant) -> bool {
        while self
            .stamps
            .front()
            .is_some_and(|first| now.duration_since(*first) >= REPEAT_WINDOW)
        {
            self.stamps.pop_front();
        }
        self.stamps.len() < MAX_REPORTS_PER_WINDOW
    }

    /// Bucht einen Befund. Nur aufzurufen, wenn [`Budget::left`] wahr war.
    fn take(&mut self, now: Instant) {
        self.stamps.push_back(now);
    }

    /// Wie viele Befunde im laufenden Fenster gebucht sind.
    fn len(&self) -> usize {
        self.stamps.len()
    }
}

/// Was zu einem Host bekannt ist.
#[derive(Debug)]
struct HostState {
    /// Die letzten Abbrüche ohne Alert, höchstens [`DROP_THRESHOLD`] Stück.
    drops: VecDeque<Instant>,
    /// Je Hinweis: wann zuletzt gemeldet wurde und wie viel seitdem auflief.
    ///
    /// Höchstens so viele Einträge, wie [`ToolHint`] Ausprägungen hat.
    rejected: HashMap<ToolHint, Rejections>,
    /// Wann [`TLS_002`] zuletzt gemeldet wurde.
    repeated: Option<Instant>,
    /// Wann [`TLS_003`] zuletzt gemeldet wurde.
    no_sni: Option<Instant>,
    /// Wann dieser Eintrag zuletzt berührt wurde; entscheidet, wer weichen muss.
    touched: Instant,
}

/// Die Ablehnungen eines Werkzeugs zu einem Host.
#[derive(Debug, Default)]
struct Rejections {
    /// Wann zuletzt eine Karte dafür herausging.
    last_report: Option<Instant>,
    /// Wie viele Ablehnungen seitdem gezählt wurden, die noch nicht in einer
    /// Karte standen.
    pending: u32,
}

impl HostState {
    fn new(now: Instant) -> Self {
        Self {
            drops: VecDeque::new(),
            rejected: HashMap::new(),
            repeated: None,
            no_sni: None,
            touched: now,
        }
    }

    /// Wahr, wenn dieser Eintrag aus jedem Fenster gelaufen ist.
    ///
    /// Dann ist auch seine Entdopplung abgelaufen: Ihn wegzuwerfen ändert
    /// nichts an dem, was gemeldet würde.
    fn is_stale(&self, now: Instant) -> bool {
        now.duration_since(self.touched) > REPEAT_WINDOW
    }
}

impl HandshakeWatch {
    /// Ein leeres Fenster.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Meldet eine ausgesprochene Ablehnung.
    ///
    /// `None` heißt: nicht berichten. `Some(n)` heißt: berichten, und `n` ist
    /// die Zahl der Ablehnungen dieser Paarung seit der letzten Karte, die
    /// jetzige eingeschlossen. Beim ersten Mal ist `n` gleich 1.
    ///
    /// Entdoppelt je Host und Hinweis für [`REPEAT_WINDOW`]: Ein Werkzeug in
    /// einer Wiederholschleife erzeugt eine Karte, nicht dreißig. Was dabei
    /// unterdrückt wird, geht in `n` der nächsten Karte ein.
    pub fn on_rejection(&self, host: &HostName, hint: ToolHint) -> Option<u32> {
        self.on_rejection_at(host, hint, Instant::now())
    }

    /// Wie [`HandshakeWatch::on_rejection`], mit gesetzter Uhr (Test).
    pub fn on_rejection_at(&self, host: &HostName, hint: ToolHint, now: Instant) -> Option<u32> {
        let mut watch = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let budget = watch.warnings.left(now);
        let state = watch.host(host, now);
        let counter = state.rejected.entry(hint).or_default();
        counter.pending = counter.pending.saturating_add(1);
        let due = counter
            .last_report
            .is_none_or(|last| now.duration_since(last) >= REPEAT_WINDOW);
        // Ohne Budget wird nichts vermerkt: Der Zähler läuft weiter, und die
        // nächste Karte nennt auch die Versuche, die hier unterdrückt wurden.
        if !due || !budget {
            return None;
        }
        counter.last_report = Some(now);
        let count = core::mem::replace(&mut counter.pending, 0);
        watch.warnings.take(now);
        Some(count)
    }

    /// Meldet einen Abbruch ohne Alert; wahr, wenn [`TLS_002`] fällig ist.
    ///
    /// Fällig ist er, sobald [`DROP_THRESHOLD`] Abbrüche in [`DROP_WINDOW`]
    /// liegen, der letzte Bericht [`REPEAT_WINDOW`] her ist und das Budget des
    /// Fensters noch reicht.
    pub fn on_drop(&self, host: &HostName) -> bool {
        self.on_drop_at(host, Instant::now())
    }

    /// Wie [`HandshakeWatch::on_drop`], mit gesetzter Uhr (Test).
    pub fn on_drop_at(&self, host: &HostName, now: Instant) -> bool {
        let mut watch = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        let budget = watch.warnings.left(now);
        let state = watch.host(host, now);
        while state
            .drops
            .front()
            .is_some_and(|first| now.duration_since(*first) > DROP_WINDOW)
        {
            state.drops.pop_front();
        }
        state.drops.push_back(now);
        // Mehr als der Schwellwert sagt nichts Neues; der älteste Zeitpunkt
        // fällt weg, damit die Warteschlange nicht mit den Versuchen wächst.
        while state.drops.len() > DROP_THRESHOLD {
            state.drops.pop_front();
        }
        if state.drops.len() < DROP_THRESHOLD {
            return false;
        }
        let due = state
            .repeated
            .is_none_or(|last| now.duration_since(last) >= REPEAT_WINDOW);
        if !due || !budget {
            return false;
        }
        state.repeated = Some(now);
        watch.warnings.take(now);
        true
    }

    /// Meldet einen Handschlag ohne SNI; wahr, wenn [`TLS_003`] fällig ist.
    pub fn on_missing_sni(&self, host: &HostName) -> bool {
        self.on_missing_sni_at(host, Instant::now())
    }

    /// Wie [`HandshakeWatch::on_missing_sni`], mit gesetzter Uhr (Test).
    pub fn on_missing_sni_at(&self, host: &HostName, now: Instant) -> bool {
        let mut watch = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
        // Aus dem Topf der Informationen, nie aus dem der Warnungen.
        let budget = watch.infos.left(now);
        let state = watch.host(host, now);
        let due = state
            .no_sni
            .is_none_or(|last| now.duration_since(last) >= REPEAT_WINDOW);
        if !due || !budget {
            return false;
        }
        state.no_sni = Some(now);
        watch.infos.take(now);
        true
    }

    /// Wie viele Hosts das Fenster gerade hält.
    #[must_use]
    pub fn tracked_hosts(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .hosts
            .len()
    }

    /// Wie viele Warnungen (`TLS_001`, `TLS_002`) im laufenden
    /// [`REPEAT_WINDOW`] herausgingen.
    #[must_use]
    pub fn warnings_in_window(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .warnings
            .len()
    }

    /// Wie viele Informationen (`TLS_003`) im laufenden [`REPEAT_WINDOW`]
    /// herausgingen.
    #[must_use]
    pub fn infos_in_window(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .infos
            .len()
    }
}

impl Watch {
    /// Der Eintrag dieses Hosts, angelegt falls nötig, und Platz dafür.
    fn host(&mut self, host: &HostName, now: Instant) -> &mut HostState {
        let key = host.to_string();
        if !self.hosts.contains_key(&key) {
            make_room(&mut self.hosts, now);
        }
        let state = self.hosts.entry(key).or_insert_with(|| HostState::new(now));
        state.touched = now;
        state
    }
}

/// Schafft Platz für einen weiteren Host: erst das Abgelaufene, dann das
/// Älteste.
///
/// Ein Client, der Handschläge zu immer neuen Namen abbricht, hält damit nie
/// mehr als [`MAX_TRACKED_HOSTS`] Einträge am Leben. Zuerst fällt weg, was
/// ohnehin aus jedem Fenster gelaufen ist; dabei geht keine Entdopplung
/// verloren. Erst wenn das nicht reicht, muss ein noch lebender Eintrag
/// weichen — dessen Entdopplung fängt der Deckel
/// [`MAX_REPORTS_PER_WINDOW`] auf.
fn make_room(hosts: &mut HashMap<String, HostState>, now: Instant) {
    if hosts.len() < MAX_TRACKED_HOSTS {
        return;
    }
    hosts.retain(|_host, state| !state.is_stale(now));
    while hosts.len() >= MAX_TRACKED_HOSTS {
        let Some(oldest) = hosts
            .iter()
            .min_by_key(|(_host, state)| state.touched)
            .map(|(host, _state)| host.clone())
        else {
            return;
        };
        hosts.remove(&oldest);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io;
    use std::time::{Duration, Instant};

    use humanitl_core::{FixAction, HeaderMap, HostName, Severity};

    use super::{
        DROP_THRESHOLD, HandshakeWatch, MAX_REPORTS_PER_WINDOW, MAX_TRACKED_HOSTS, TlsFailure,
        ToolHint, classify, diagnostic_for, missing_sni, tool_hint,
    };

    fn host(name: &str) -> HostName {
        HostName::parse(name).unwrap_or_else(|err| panic!("{err}"))
    }

    fn agent(value: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if !value.is_empty() {
            headers.insert(
                http::header::USER_AGENT,
                http::HeaderValue::from_str(value).unwrap_or_else(|err| panic!("{err}")),
            );
        }
        headers
    }

    /// So verpackt `tokio-rustls` einen `rustls::Error`.
    fn wrapped(err: rustls::Error) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, err)
    }

    #[test]
    fn classify_unknown_ca() {
        let err = wrapped(rustls::Error::AlertReceived(
            rustls::AlertDescription::UnknownCA,
        ));
        assert_eq!(classify(&err), Some(TlsFailure::AlertUnknownCa));
        assert!(
            classify(&err)
                .unwrap_or_else(|| panic!("classified"))
                .is_rejection()
        );

        let err = wrapped(rustls::Error::AlertReceived(
            rustls::AlertDescription::BadCertificate,
        ));
        assert_eq!(classify(&err), Some(TlsFailure::AlertBadCertificate));
    }

    #[test]
    fn classify_eof() {
        let err = io::Error::new(io::ErrorKind::UnexpectedEof, "tls handshake eof");
        assert_eq!(classify(&err), Some(TlsFailure::EofBeforeFinished));

        for kind in [
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::ConnectionAborted,
            // Der häufige Fall: Der Client legt nach dem `ClientHello` auf, und
            // der Proxy bricht sich beim Schreiben der Antwort das Rohr.
            io::ErrorKind::BrokenPipe,
        ] {
            let err = io::Error::from(kind);
            assert_eq!(
                classify(&err),
                Some(TlsFailure::ResetBeforeFinished),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn an_alert_that_says_nothing_about_the_ca_is_not_a_rejection() {
        let err = wrapped(rustls::Error::AlertReceived(
            rustls::AlertDescription::HandshakeFailure,
        ));
        let failure = classify(&err).unwrap_or_else(|| panic!("classified"));
        assert!(!failure.is_rejection(), "{failure:?}");
        assert!(failure.is_drop());
    }

    #[test]
    fn an_error_that_says_nothing_about_the_client_stays_unclassified() {
        let err = io::Error::new(io::ErrorKind::WriteZero, "no room");
        assert_eq!(classify(&err), None);
        let err = wrapped(rustls::Error::NoCertificatesPresented);
        assert_eq!(classify(&err), None);
    }

    #[test]
    fn tool_hint_from_user_agent() {
        for (value, expected) in [
            ("curl/8.5.0", ToolHint::Curl),
            ("node", ToolHint::Node),
            ("Bun/1.1.0", ToolHint::Bun),
            ("python-requests/2.31.0", ToolHint::Python),
            ("Go-http-client/1.1", ToolHint::Go),
            ("Java/21", ToolHint::Java),
            ("git/2.43.0", ToolHint::Git),
            ("", ToolHint::Unknown),
            ("GitHub-Hookshot/1.0", ToolHint::Unknown),
            ("Mozilla/5.0 (X11; Linux x86_64)", ToolHint::Unknown),
        ] {
            assert_eq!(tool_hint(&agent(value)), expected, "user-agent {value:?}");
        }
    }

    #[test]
    fn the_rejection_names_host_and_variable_but_never_the_user_agent() {
        let diagnostic = diagnostic_for(
            &TlsFailure::AlertUnknownCa,
            &host("example.com"),
            ToolHint::Curl,
            1,
        );
        assert_eq!(diagnostic.code.as_str(), "TLS_001");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(diagnostic.why.contains("example.com"), "{}", diagnostic.why);
        assert!(
            diagnostic.why.contains("calls itself curl"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::SetEnv {
                key: "CURL_CA_BUNDLE".to_owned(),
                value: "/etc/humanitl/ca.crt".to_owned(),
            })
        );
    }

    #[test]
    fn the_rejection_does_not_claim_a_missing_variable() {
        // Jedes ausgelieferte Profil setzt die Variable, die der Fix
        // vorschlägt (`profiles/sandbox/default.toml`, `ENV_KIT`). Belegt ist
        // nur, dass der Client das Leaf verworfen hat.
        for hint in [ToolHint::Curl, ToolHint::Go, ToolHint::Unknown] {
            let diagnostic =
                diagnostic_for(&TlsFailure::AlertUnknownCa, &host("example.com"), hint, 1);
            let why = &diagnostic.why;
            assert!(
                why.contains("Humanitl already sets"),
                "{hint:?} must say the variable is already set: {why}"
            );
            assert!(
                !why.contains("this one did not"),
                "{hint:?} must not claim a missing variable: {why}"
            );
            let key = hint
                .ca_variable()
                .unwrap_or_else(|| panic!("{hint:?} has a variable"));
            assert!(why.contains(key), "{hint:?}: {why}");
            // Die vierte Ursache: Das Werkzeug liest die Variable gar nicht.
            assert!(
                why.contains(&format!("does not read {key} at all")),
                "{hint:?} must name the fourth cause: {why}"
            );
            // Der Knopf schreibt nach `[sandbox.env]` in `config.toml`, nicht
            // in ein Sandbox-Profil; genau das trägt die Variable ja schon.
            assert!(
                why.contains("under [sandbox.env] in config.toml"),
                "{hint:?} must name where the button writes: {why}"
            );
            assert!(
                !why.contains("in your global profile"),
                "{hint:?} must not send anyone to the profile file: {why}"
            );
        }
    }

    #[test]
    fn the_repeat_count_is_named_with_its_unit_from_the_second_card_on() {
        let first = diagnostic_for(
            &TlsFailure::AlertUnknownCa,
            &host("example.com"),
            ToolHint::Curl,
            1,
        );
        assert!(
            !first.why.contains("counted since"),
            "the first card counts nothing: {}",
            first.why
        );
        let later = diagnostic_for(
            &TlsFailure::AlertUnknownCa,
            &host("example.com"),
            ToolHint::Curl,
            7,
        );
        assert!(
            later.why.contains(
                "7 rejected handshakes have been counted for this host and this tool hint since \
                 the previous card"
            ),
            "the counter names what it counted, host and tool hint: {}",
            later.why
        );
    }

    #[test]
    fn an_unknown_tool_gets_the_generic_variable_and_no_claim() {
        let diagnostic = diagnostic_for(
            &TlsFailure::AlertBadCertificate,
            &host("example.com"),
            ToolHint::Unknown,
            1,
        );
        assert!(
            diagnostic.why.starts_with("A client inside the sandbox"),
            "{}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::SetEnv {
                key: "SSL_CERT_FILE".to_owned(),
                value: "/etc/humanitl/ca.crt".to_owned(),
            })
        );
    }

    #[test]
    fn the_jvm_gets_an_explanation_instead_of_a_broken_button() {
        let diagnostic = diagnostic_for(
            &TlsFailure::AlertUnknownCa,
            &host("example.com"),
            ToolHint::Java,
            1,
        );
        assert_eq!(diagnostic.fix, None);
        assert!(
            diagnostic.why.contains("Java truststore"),
            "{}",
            diagnostic.why
        );
        assert!(
            !diagnostic.why.contains("Humanitl already sets"),
            "there is no variable for the JVM to have set: {}",
            diagnostic.why
        );
    }

    #[test]
    fn python_names_the_second_variable_the_button_cannot_carry() {
        // `FixAction::SetEnv` trägt genau ein Paar; die zweite Variable steht
        // deshalb im Text (Abweichung von der Tabelle der Spezifikation).
        let diagnostic = diagnostic_for(
            &TlsFailure::AlertUnknownCa,
            &host("example.com"),
            ToolHint::Python,
            1,
        );
        assert!(
            diagnostic.why.contains("REQUESTS_CA_BUNDLE"),
            "{}",
            diagnostic.why
        );
        assert!(
            diagnostic.why.contains(
                "Humanitl sets both in the sandbox, and the fix below carries only the first"
            ),
            "the note must not send anyone to a variable that is already set: {}",
            diagnostic.why
        );
        assert!(
            !diagnostic.why.contains("both have to be set"),
            "REQUESTS_CA_BUNDLE is in ENV_KIT; asking for it by hand would be false: {}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::SetEnv {
                key: "SSL_CERT_FILE".to_owned(),
                value: "/etc/humanitl/ca.crt".to_owned(),
            })
        );
    }

    #[test]
    fn a_drop_becomes_tls_002_with_a_block_rule() {
        let diagnostic = diagnostic_for(
            &TlsFailure::EofBeforeFinished,
            &host("example.com"),
            ToolHint::Unknown,
            1,
        );
        assert_eq!(diagnostic.code.as_str(), "TLS_002");
        assert!(
            matches!(diagnostic.fix, Some(FixAction::AddRule(_))),
            "{:?}",
            diagnostic.fix
        );
    }

    #[test]
    fn missing_sni_is_information_without_a_fix() {
        let diagnostic = missing_sni(&host("example.com"));
        assert_eq!(diagnostic.code.as_str(), "TLS_003");
        assert_eq!(diagnostic.severity, Severity::Info);
        assert_eq!(diagnostic.fix, None);
    }

    #[test]
    fn tls_002_after_three_resets() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let target = host("example.com");
        assert!(!watch.on_drop_at(&target, start));
        assert!(!watch.on_drop_at(&target, start + Duration::from_secs(1)));
        assert!(
            watch.on_drop_at(&target, start + Duration::from_secs(2)),
            "three drops within ten seconds are a pattern"
        );
        // Innerhalb der Minute wird nicht wiederholt, auch wenn das Muster
        // weiter besteht.
        assert!(!watch.on_drop_at(&target, start + Duration::from_secs(3)));
        // Danach schon, sobald das Muster sich neu gebildet hat: Die alten
        // Zeitpunkte sind aus dem Zehn-Sekunden-Fenster gelaufen.
        let later = start + Duration::from_secs(62);
        assert!(!watch.on_drop_at(&target, later));
        assert!(!watch.on_drop_at(&target, later + Duration::from_secs(1)));
        assert!(watch.on_drop_at(&target, later + Duration::from_secs(2)));
    }

    #[test]
    fn drops_that_are_too_far_apart_are_no_pattern() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let target = host("example.com");
        for step in 0..DROP_THRESHOLD + 2 {
            let now = start + Duration::from_secs(step as u64 * 11);
            assert!(!watch.on_drop_at(&target, now), "step {step}");
        }
    }

    #[test]
    fn a_rejection_is_reported_once_per_host_and_hint() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let target = host("example.com");
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start),
            Some(1)
        );
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start + Duration::from_secs(5)),
            None
        );
        // Ein anderes Werkzeug ist ein anderer Befund.
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Node, start + Duration::from_secs(5)),
            Some(1)
        );
        // Ein anderer Host auch.
        assert_eq!(
            watch.on_rejection_at(&host("other.example"), ToolHint::Curl, start),
            Some(1)
        );
    }

    #[test]
    fn the_suppressed_attempts_are_counted_into_the_next_card() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let target = host("example.com");
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start),
            Some(1)
        );
        for step in 1..4 {
            assert_eq!(
                watch.on_rejection_at(&target, ToolHint::Curl, start + Duration::from_secs(step)),
                None,
                "step {step}"
            );
        }
        // Drei unterdrückte Versuche plus dieser: die Karte nennt vier.
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start + Duration::from_secs(61)),
            Some(4)
        );
    }

    #[test]
    fn the_report_budget_holds_even_when_the_hosts_rotate() {
        // Ein Client, der über mehr Namen rotiert, als das Fenster verfolgt:
        // Die Verdrängung nimmt die Entdopplung des verdrängten Eintrags mit,
        // der Deckel über allen Hosts hält die Zahl der Karten trotzdem.
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let mut cards = 0;
        for index in 0..(MAX_TRACKED_HOSTS * 4) {
            let target = host(&format!("h{index}.example"));
            let now = start + Duration::from_millis(index as u64);
            if watch
                .on_rejection_at(&target, ToolHint::Curl, now)
                .is_some()
            {
                cards += 1;
            }
            assert!(watch.tracked_hosts() <= MAX_TRACKED_HOSTS);
            assert!(watch.warnings_in_window() <= MAX_REPORTS_PER_WINDOW);
        }
        assert_eq!(
            cards, MAX_REPORTS_PER_WINDOW,
            "a rotating client gets the budget of one window, not a card per name"
        );
        // Nach dem Fenster gibt es wieder ein volles Budget.
        assert!(
            watch
                .on_rejection_at(
                    &host("fresh.example"),
                    ToolHint::Curl,
                    start + Duration::from_secs(61),
                )
                .is_some()
        );
    }

    #[test]
    fn an_information_cannot_starve_the_warning() {
        // Der gemessene Ablauf: Der Client in der Sandbox waehlt Zielnamen und
        // SNI selbst und braucht dafuer kein DNS. Mit einem gemeinsamen Topf
        // koennte er ihn mit TLS_003 leerlaufen lassen und damit fuer eine
        // Minute unterdruecken, dass ein Mensch von einem abgelehnten
        // Zertifikat erfaehrt.
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let mut infos = 0;
        for index in 0..(MAX_REPORTS_PER_WINDOW * 4) {
            let target = host(&format!("made-up-{index}.example"));
            if watch.on_missing_sni_at(&target, start + Duration::from_millis(index as u64)) {
                infos += 1;
            }
        }
        assert_eq!(
            infos, MAX_REPORTS_PER_WINDOW,
            "the info budget is spent, as intended"
        );
        assert_eq!(watch.infos_in_window(), MAX_REPORTS_PER_WINDOW);
        assert_eq!(watch.warnings_in_window(), 0, "and it cost no warning");

        // Und jetzt die Karte, um die es geht.
        assert_eq!(
            watch.on_rejection_at(
                &host("api.github.com"),
                ToolHint::Curl,
                start + Duration::from_secs(1),
            ),
            Some(1),
            "a rejected certificate must still reach the person"
        );
    }

    #[test]
    fn a_warning_cannot_starve_the_information_either() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        for index in 0..(MAX_REPORTS_PER_WINDOW * 2) {
            let target = host(&format!("h{index}.example"));
            let _card = watch.on_rejection_at(
                &target,
                ToolHint::Curl,
                start + Duration::from_millis(index as u64),
            );
        }
        assert_eq!(watch.warnings_in_window(), MAX_REPORTS_PER_WINDOW);
        assert!(
            watch.on_missing_sni_at(&host("api.github.com"), start + Duration::from_secs(1)),
            "the two budgets are independent in both directions"
        );
    }

    #[test]
    fn an_evicted_host_starts_its_count_again() {
        // Was der Modulkommentar zusagt, und nicht mehr: Der Zaehler lebt mit
        // seinem Eintrag. Wird der verdraengt, faengt er wieder bei eins an;
        // die History behaelt trotzdem jeden Versuch.
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        let target = host("api.github.com");
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start),
            Some(1)
        );
        for step in 1..7 {
            assert_eq!(
                watch.on_rejection_at(&target, ToolHint::Curl, start + Duration::from_secs(step)),
                None
            );
        }
        // Rotation ueber genug Namen, um den lebenden Eintrag zu verdraengen.
        for index in 0..(MAX_TRACKED_HOSTS * 2) {
            let other = host(&format!("r{index}.example"));
            let _card = watch.on_rejection_at(
                &other,
                ToolHint::Curl,
                start + Duration::from_millis(10_000 + index as u64),
            );
        }
        assert_eq!(
            watch.on_rejection_at(&target, ToolHint::Curl, start + Duration::from_secs(61)),
            Some(1),
            "the counter is gone with its entry; the module comment says so"
        );
    }

    #[test]
    fn the_window_never_grows_past_its_cap() {
        let watch = HandshakeWatch::new();
        let start = Instant::now();
        for index in 0..(MAX_TRACKED_HOSTS * 3) {
            let target = host(&format!("h{index}.example"));
            // Alle innerhalb derselben Sekunde: nichts läuft ab, es muss also
            // der am längsten nicht gesehene Eintrag weichen.
            let _reported = watch.on_drop_at(&target, start + Duration::from_millis(index as u64));
            assert!(
                watch.tracked_hosts() <= MAX_TRACKED_HOSTS,
                "the window grew to {} entries",
                watch.tracked_hosts()
            );
        }
        assert_eq!(watch.tracked_hosts(), MAX_TRACKED_HOSTS);
    }
}
