//! Regeln als reine Werttypen.
//!
//! Hier stehen Aufbau und Invarianten einer Regel, nicht ihre Auswertung. Das
//! Einlesen von `rules.yaml`, die Vorrangordnung und `RuleSet::evaluate` liegen
//! in `humanitl-rules`; das Muster für einen Katalogeintrag benutzt
//! `humanitl-catalog`. Beide sähen sonst dieselbe Struktur zweimal, und
//! `FixAction::AddRule` aus [`crate::diagnostics`] könnte gar keine Regel
//! vorschlagen, ohne einen Abhängigkeitszyklus zu bauen.

use core::fmt;
use core::str::FromStr;
use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::host::HostName;
use crate::http::{Method, Scheme};
use crate::ids::{FlowId, RuleId, SessionId};

pub use crate::http::Upgrade;

/// Was mit einer passenden Anfrage geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Durchlassen, ohne zu fragen.
    Allow,
    /// Blocken, ohne zu fragen.
    Block,
    /// Den Menschen fragen (der Standard ohne Regel).
    Ask,
    /// Durchlassen, aber Funde vorher ersetzen.
    Redact,
}

impl Action {
    /// Kurzname in `snake_case`, wie er in `rules.yaml` steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Block => "block",
            Self::Ask => "ask",
            Self::Redact => "redact",
        }
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Wie lange eine Regel gilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Expiry {
    /// Dauerhaft, bis der Nutzer sie löscht.
    Never,
    /// Nur für diese Sitzung; nicht in `rules.yaml`.
    Session(SessionId),
    /// Bis zu einem Zeitpunkt.
    At(DateTime<Utc>),
}

impl Expiry {
    /// Wahr, wenn die Regel zu diesem Zeitpunkt in dieser Sitzung nicht mehr gilt.
    ///
    /// Eine Sitzungsregel einer anderen Sitzung gilt nie. `At` läuft mit
    /// Erreichen des Zeitpunkts ab.
    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>, session: SessionId) -> bool {
        match self {
            Self::Never => false,
            Self::Session(owner) => *owner != session,
            Self::At(deadline) => *deadline <= now,
        }
    }
}

/// Ein Text ließ sich nicht als Host-Muster lesen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid host pattern {input:?}: {reason}")]
pub struct HostPatternError {
    /// Der abgelehnte Text, unverändert.
    pub input: String,
    /// Warum der Text abgelehnt wurde.
    pub reason: &'static str,
}

impl HostPatternError {
    fn new(input: &str, reason: &'static str) -> Self {
        Self {
            input: input.to_owned(),
            reason,
        }
    }
}

/// Muster für den Host einer Regel.
///
/// Die Auswertung steht in `humanitl-rules`. Hier gilt nur die Invariante: die
/// Labels sind normalisiert (A-Label, klein), Platzhalter stehen als ganzes
/// Label, und eine Adresse ist entweder eine einzelne IP oder ein Netz.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostPattern {
    /// Genau dieser Host, ohne Platzhalter.
    Exact(HostName),
    /// Label-Glob, zum Beispiel `*.github.com` oder `**.github.com`.
    ///
    /// `*` steht für genau ein Label, `**` für ein oder mehrere Labels;
    /// `**.example.com` schließt `example.com` selbst ein.
    Glob(String),
    /// Genau diese Adresse, geschrieben als `ip:192.168.1.50`.
    Ip(IpAddr),
    /// Ein Netz, geschrieben als `cidr:192.168.0.0/16`.
    Cidr {
        /// Die Netzadresse.
        addr: IpAddr,
        /// Länge des Präfixes in Bit.
        prefix: u8,
    },
}

impl HostPattern {
    /// Liest ein Muster aus der Schreibweise von `rules.yaml`.
    ///
    /// `ip:` und `cidr:` führen zu Adressmustern, ein `*` irgendwo zu einem
    /// Glob, alles andere zu einem exakten Host. Nicht-Platzhalter-Labels
    /// laufen durch dieselbe Normalisierung wie [`HostName::parse`].
    ///
    /// # Errors
    ///
    /// [`HostPatternError`], wenn Text weder Host, noch Glob, noch Adresse ist.
    pub fn parse(input: &str) -> Result<Self, HostPatternError> {
        if let Some(rest) = input.strip_prefix("ip:") {
            return rest
                .parse::<IpAddr>()
                .map(Self::Ip)
                .map_err(|_| HostPatternError::new(input, "not an ip address"));
        }
        if let Some(rest) = input.strip_prefix("cidr:") {
            let (addr, prefix) = rest
                .split_once('/')
                .ok_or_else(|| HostPatternError::new(input, "cidr needs a prefix length"))?;
            let addr = addr
                .parse::<IpAddr>()
                .map_err(|_| HostPatternError::new(input, "not an ip address"))?;
            let prefix = prefix
                .parse::<u8>()
                .map_err(|_| HostPatternError::new(input, "prefix length is not a number"))?;
            return Self::cidr(addr, prefix)
                .map_err(|_| HostPatternError::new(input, "prefix length out of range"));
        }
        if input.contains('*') {
            return Self::glob(input);
        }
        HostName::parse(input)
            .map(Self::Exact)
            .map_err(|err| HostPatternError::new(input, err.reason))
    }

    /// Baut ein Glob-Muster und normalisiert seine festen Labels.
    ///
    /// # Errors
    ///
    /// [`HostPatternError`], wenn ein Label leer ist, ein Platzhalter mit Text
    /// im selben Label steht (`*a.example.com`) oder ein festes Label kein
    /// gültiges Label ist.
    pub fn glob(input: &str) -> Result<Self, HostPatternError> {
        if input.is_empty() {
            return Err(HostPatternError::new(input, "empty pattern"));
        }
        let mut labels = Vec::new();
        for label in input.split('.') {
            match label {
                "*" | "**" => labels.push(label.to_owned()),
                "" => return Err(HostPatternError::new(input, "empty label")),
                _ if label.contains('*') => {
                    return Err(HostPatternError::new(
                        input,
                        "a wildcard must be a whole label",
                    ));
                }
                _ => {
                    let ascii = idna::domain_to_ascii_strict(label)
                        .map_err(|_| HostPatternError::new(input, "not a valid label"))?;
                    labels.push(ascii);
                }
            }
        }
        Ok(Self::Glob(labels.join(".")))
    }

    /// Baut ein Netz-Muster.
    ///
    /// # Errors
    ///
    /// [`HostPatternError`], wenn die Präfixlänge nicht zur Adressfamilie passt.
    pub fn cidr(addr: IpAddr, prefix: u8) -> Result<Self, HostPatternError> {
        let max = if addr.is_ipv4() { 32 } else { 128 };
        if prefix > max {
            return Err(HostPatternError::new(
                &format!("cidr:{addr}/{prefix}"),
                "prefix length out of range for this address family",
            ));
        }
        Ok(Self::Cidr { addr, prefix })
    }

    /// Wahr, wenn das Muster einen Platzhalter enthält.
    #[must_use]
    pub const fn is_glob(&self) -> bool {
        matches!(self, Self::Glob(_))
    }
}

impl fmt::Display for HostPattern {
    /// Die Schreibweise, die auch [`HostPattern::parse`] wieder liest.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exact(host) => write!(f, "{host}"),
            Self::Glob(pattern) => f.write_str(pattern),
            Self::Ip(addr) => write!(f, "ip:{addr}"),
            Self::Cidr { addr, prefix } => write!(f, "cidr:{addr}/{prefix}"),
        }
    }
}

impl FromStr for HostPattern {
    type Err = HostPatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

/// Muster für den Pfad einer Regel.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PathPattern {
    /// Glob über den Pfad, zum Beispiel `/repos/**`.
    Glob(String),
    /// Regulärer Ausdruck, in `rules.yaml` mit `~` eingeleitet.
    Regex(String),
}

impl PathPattern {
    /// Liest ein Pfadmuster aus der Schreibweise von `rules.yaml`.
    ///
    /// Ein führendes `~` macht den Rest zu einem regulären Ausdruck. Übersetzt
    /// wird das Muster erst in `humanitl-rules`; ein ungültiger Ausdruck fällt
    /// dort auf, nicht hier.
    #[must_use]
    pub fn parse(input: &str) -> Self {
        input.strip_prefix('~').map_or_else(
            || Self::Glob(input.to_owned()),
            |rest| Self::Regex(rest.to_owned()),
        )
    }
}

impl fmt::Display for PathPattern {
    /// Die Schreibweise, die auch [`PathPattern::parse`] wieder liest.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Glob(pattern) => f.write_str(pattern),
            Self::Regex(pattern) => write!(f, "~{pattern}"),
        }
    }
}

/// Bedingung einer Regel.
///
/// Ein Feld auf `None` heißt „egal". Der Host ist immer gesetzt: eine Regel
/// ohne Host gäbe es nicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matcher {
    /// Muster für den Host.
    pub host: HostPattern,
    /// Erlaubte Methoden; `None` heißt jede.
    pub methods: Option<Vec<Method>>,
    /// Muster für Pfad und Query.
    pub path: Option<PathPattern>,
    /// Verlangtes Schema.
    pub scheme: Option<Scheme>,
    /// Verlangter Port.
    pub port: Option<u16>,
    /// Verlangter Protokollwechsel; `None` heißt „kein Upgrade".
    pub upgrade: Option<Upgrade>,
}

impl Matcher {
    /// Eine Bedingung, die nur den Host prüft.
    #[must_use]
    pub const fn host(host: HostPattern) -> Self {
        Self {
            host,
            methods: None,
            path: None,
            scheme: None,
            port: None,
            upgrade: None,
        }
    }

    /// Schränkt auf diese Methoden ein.
    #[must_use]
    pub fn with_methods(mut self, methods: Vec<Method>) -> Self {
        self.methods = Some(methods);
        self
    }

    /// Schränkt auf dieses Pfadmuster ein.
    #[must_use]
    pub fn with_path(mut self, path: PathPattern) -> Self {
        self.path = Some(path);
        self
    }

    /// Schränkt auf dieses Schema ein.
    #[must_use]
    pub const fn with_scheme(mut self, scheme: Scheme) -> Self {
        self.scheme = Some(scheme);
        self
    }

    /// Schränkt auf diesen Port ein.
    #[must_use]
    pub const fn with_port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Schränkt auf diesen Protokollwechsel ein.
    #[must_use]
    pub const fn with_upgrade(mut self, upgrade: Upgrade) -> Self {
        self.upgrade = Some(upgrade);
        self
    }
}

/// Eine Regel: Bedingung, Aktion, Gültigkeit, Herkunft.
#[derive(Debug, Clone, PartialEq, Eq)]
// Vier Wahrheitswerte, und clippy schlägt einen Zustandstyp vor. Er wäre hier
// falsch: `stream`, `allow_private`, `bundled` und `disabled` sind vier
// unabhängige Eigenschaften derselben Regel, jede mit ihrem eigenen Schlüssel
// in `rules.yaml` (`backlog/CONVENTIONS.md` 3.3). Ein Aufzählungstyp behauptete
// eine Ordnung oder einen Ausschluss zwischen ihnen, den es nicht gibt, und die
// Datei ließe sich nicht mehr eins zu eins abbilden.
#[allow(clippy::struct_excessive_bools)]
pub struct Rule {
    /// Id der Regel.
    pub id: RuleId,
    /// Was mit einer passenden Anfrage geschieht.
    pub action: Action,
    /// Wann die Regel passt.
    pub matcher: Matcher,
    /// Wie lange die Regel gilt.
    pub expires: Expiry,
    /// Ob ein Body über dem Cap gestreamt statt geblockt wird.
    pub stream: bool,
    /// Ob Ziele in privaten Netzen erlaubt sind.
    ///
    /// Ohne dieses Recht verweigert der Proxy eine aufgelöste Adresse aus
    /// RFC 1918, Loopback, Link-Local oder CGNAT. Die Passthrough-Regel für
    /// einen lokalen LLM-Endpunkt setzt es.
    pub allow_private: bool,
    /// Aus welcher Entscheidung die Regel entstanden ist.
    pub created_from: Option<FlowId>,
    /// Ob die Regel mitgeliefert wurde statt vom Nutzer angelegt.
    pub bundled: bool,
    /// Ob die Regel abgeschaltet ist.
    ///
    /// Eine abgeschaltete Regel bleibt im Regelsatz stehen und wird bei der
    /// Auswertung übersprungen. Nur so lässt sich eine mitgelieferte Regel
    /// aufheben, ohne sie zu löschen: sie gehört nicht dem Nutzer (`RULES_010`),
    /// bleibt aber im Rules-Screen sichtbar samt ihrer Begründung. Persistiert
    /// wird der Zustand in der `rules.yaml` des Nutzers als Liste
    /// `disabled_bundled`, nie in `rules/default.yaml` (HUM-038).
    pub disabled: bool,
    /// Freitext des Nutzers.
    pub note: Option<String>,
}

impl Rule {
    /// Eine dauerhafte Regel mit den üblichen Vorgaben.
    #[must_use]
    pub const fn new(id: RuleId, action: Action, matcher: Matcher) -> Self {
        Self {
            id,
            action,
            matcher,
            expires: Expiry::Never,
            stream: false,
            allow_private: false,
            created_from: None,
            bundled: false,
            disabled: false,
            note: None,
        }
    }

    /// Setzt die Gültigkeit.
    #[must_use]
    pub const fn with_expiry(mut self, expires: Expiry) -> Self {
        self.expires = expires;
        self
    }

    /// Erlaubt Ziele in privaten Netzen.
    #[must_use]
    pub const fn with_allow_private(mut self, allow_private: bool) -> Self {
        self.allow_private = allow_private;
        self
    }

    /// Erlaubt das Streamen großer Bodies.
    #[must_use]
    pub const fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Vermerkt, aus welcher Entscheidung die Regel entstanden ist.
    #[must_use]
    pub const fn created_from(mut self, flow: FlowId) -> Self {
        self.created_from = Some(flow);
        self
    }

    /// Markiert die Regel als mitgeliefert.
    #[must_use]
    pub const fn bundled(mut self, bundled: bool) -> Self {
        self.bundled = bundled;
        self
    }

    /// Schaltet die Regel ab oder wieder an.
    #[must_use]
    pub const fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Setzt die Notiz.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Wahr, wenn die Regel zu diesem Zeitpunkt in dieser Sitzung nicht mehr gilt.
    #[must_use]
    pub fn is_expired(&self, now: DateTime<Utc>, session: SessionId) -> bool {
        self.expires.is_expired(now, session)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use chrono::{TimeZone, Utc};

    use super::{Action, Expiry, HostPattern, Matcher, PathPattern, Rule};
    use crate::ids::{RuleId, SessionId};

    #[test]
    fn host_pattern_round_trips() {
        for (input, expected) in [
            ("github.com", "github.com"),
            ("*.github.com", "*.github.com"),
            ("**.github.com", "**.github.com"),
            ("MÜNCHEN.de", "xn--mnchen-3ya.de"),
            ("*.MÜNCHEN.de", "*.xn--mnchen-3ya.de"),
            ("ip:192.168.1.50", "ip:192.168.1.50"),
            ("cidr:192.168.0.0/16", "cidr:192.168.0.0/16"),
        ] {
            let parsed = HostPattern::parse(input).unwrap_or_else(|err| panic!("{input}: {err}"));
            assert_eq!(parsed.to_string(), expected, "input {input}");
        }
    }

    #[test]
    fn host_pattern_rejects_broken_input() {
        for input in [
            "*a.github.com",
            "a..b",
            "",
            "ip:not-an-ip",
            "cidr:192.168.0.0",
            "cidr:192.168.0.0/33",
            "cidr:::1/300",
        ] {
            assert!(
                HostPattern::parse(input).is_err(),
                "{input} should not parse"
            );
        }
    }

    #[test]
    fn path_pattern_distinguishes_regex() {
        assert_eq!(
            PathPattern::parse("/repos/**"),
            PathPattern::Glob("/repos/**".to_owned())
        );
        assert_eq!(
            PathPattern::parse("~^/v[0-9]+/"),
            PathPattern::Regex("^/v[0-9]+/".to_owned())
        );
        assert_eq!(PathPattern::parse("~^/v[0-9]+/").to_string(), "~^/v[0-9]+/");
    }

    #[test]
    fn expiry_depends_on_session_and_time() {
        let session = SessionId::new();
        let other = SessionId::new();
        let now = Utc.with_ymd_and_hms(2026, 9, 3, 10, 0, 0).single();
        let Some(now) = now else {
            panic!("fixed timestamp must exist");
        };

        assert!(!Expiry::Never.is_expired(now, session));
        assert!(!Expiry::Session(session).is_expired(now, session));
        assert!(Expiry::Session(other).is_expired(now, session));
        assert!(Expiry::At(now).is_expired(now, session));
        assert!(!Expiry::At(now + chrono::Duration::seconds(1)).is_expired(now, session));
    }

    #[test]
    fn rule_defaults_are_conservative() {
        let Ok(pattern) = HostPattern::parse("**.github.com") else {
            panic!("pattern must parse");
        };
        let rule = Rule::new(RuleId::new(), Action::Allow, Matcher::host(pattern));
        assert!(!rule.stream);
        assert!(!rule.allow_private);
        assert!(!rule.bundled);
        assert_eq!(rule.expires, Expiry::Never);
        assert_eq!(rule.note, None);
        let rule = rule.with_allow_private(true).with_note("LLM passthrough");
        assert!(rule.allow_private);
        assert_eq!(rule.note.as_deref(), Some("LLM passthrough"));
    }
}
