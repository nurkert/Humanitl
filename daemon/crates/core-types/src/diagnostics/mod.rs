//! Diagnostics: ein Fehler, den ein Mensch lesen und beheben kann.
//!
//! Prinzip 7 aus `BACKLOG.md`: Jeder nicht-grüne Zustand trägt Grund und Fix.
//! Damit das nicht Prosa bleibt, ist ein [`Diagnostic`] ein Wert mit festem
//! Code, fester Überschrift aus dem Register und einem `why`, das die konkreten
//! Werte des Fehlers nennt. Wo möglich hängt ein [`FixAction`] daran, den die
//! Oberfläche als Knopf und die Kommandozeile als Vorschlag zeigt.
//!
//! `Err(String)` ist in öffentlichen Signaturen der Daemon-Crates verboten;
//! `scripts/ci/lint-no-string-errors.sh` prüft das.

pub mod codes;

use core::fmt;
use std::path::PathBuf;

pub use codes::{AREAS, AreaInfo, CODES, CodeInfo, lookup, lookup_str};

use crate::rule::Rule;

/// Ein Code aus dem Register, Schema `BEREICH_NNN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiagnosticCode(pub &'static str);

impl DiagnosticCode {
    /// Die Textform, zum Beispiel `SANDBOX_001`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Wie schwer ein Befund wiegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    /// Zur Kenntnis, nichts zu tun.
    Info,
    /// Es läuft, aber nicht so, wie es soll.
    Warning,
    /// Etwas ist fehlgeschlagen.
    Error,
    /// Die Aktion wird verweigert, zum Beispiel der Start der Sandbox.
    Blocking,
}

impl Severity {
    /// Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Blocking => "blocking",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Ein Vorschlag, wie sich der Befund beheben lässt.
///
/// Ausgeführt wird nichts von allein: die Oberfläche zeigt einen Knopf, die
/// Kommandozeile zeigt den Befehl.
#[derive(Debug, Clone, PartialEq)]
pub enum FixAction {
    /// Eine Umgebungsvariable setzen.
    SetEnv {
        /// Name der Variable.
        key: String,
        /// Wert der Variable.
        value: String,
    },
    /// Eine Regel anlegen.
    AddRule(Box<Rule>),
    /// Den Dienst einrichten (`humanitl daemon install`).
    InstallService,
    /// Eine Einstellung ändern.
    ChangeSetting {
        /// Schlüssel im Schema, zum Beispiel `limits.hold_body_cap_bytes`.
        key: String,
        /// Der vorgeschlagene Wert.
        value: String,
    },
    /// Einen Befehl in die Zwischenablage legen.
    CopyCommand(String),
    /// Eine Adresse im Browser öffnen.
    OpenUrl(String),
    /// Einen Pfad nur noch lesbar einhängen.
    RemountReadOnly(PathBuf),
}

impl FixAction {
    /// Kurzname der Art in `snake_case`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::SetEnv { .. } => "set_env",
            Self::AddRule(_) => "add_rule",
            Self::InstallService => "install_service",
            Self::ChangeSetting { .. } => "change_setting",
            Self::CopyCommand(_) => "copy_command",
            Self::OpenUrl(_) => "open_url",
            Self::RemountReadOnly(_) => "remount_read_only",
        }
    }
}

/// Ein Befund mit Grund und, wenn möglich, Behebung.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{code}: {title}: {why}")]
pub struct Diagnostic {
    /// Der Code aus dem Register.
    pub code: DiagnosticCode,
    /// Wie schwer der Befund wiegt.
    pub severity: Severity,
    /// Der feste Teil der Meldung, aus dem Register.
    pub title: String,
    /// Der veränderliche Teil: die konkreten Werte dieses Falls.
    pub why: String,
    /// Was der Nutzer tun kann.
    pub fix: Option<FixAction>,
    /// Adresse mit mehr Text dazu.
    pub docs: Option<String>,
}

impl Diagnostic {
    /// Beginnt einen Befund; die Überschrift kommt aus dem Register.
    ///
    /// Ein Code, der nicht im Register steht, ist ein Fehler im Aufrufer. Im
    /// Debug-Build fällt er als `debug_assert` auf, im Release-Build wird der
    /// Code selbst zur Überschrift, damit die Meldung nicht leer ist.
    #[must_use]
    pub fn builder(code: DiagnosticCode, severity: Severity) -> DiagnosticBuilder<MissingWhy> {
        debug_assert!(
            lookup(code).is_some(),
            "diagnostic code is not in the registry"
        );
        let title =
            lookup(code).map_or_else(|| code.as_str().to_owned(), |info| info.title.to_owned());
        DiagnosticBuilder {
            code,
            severity,
            title,
            why: MissingWhy,
            fix: None,
            docs: None,
        }
    }

    /// Der Eintrag des Registers zu diesem Befund.
    #[must_use]
    pub fn info(&self) -> Option<&'static CodeInfo> {
        lookup(self.code)
    }
}

/// Markierung: das `why` fehlt noch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingWhy;

/// Markierung: das `why` steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HasWhy(String);

/// Baut einen [`Diagnostic`].
///
/// Der Typparameter hält fest, ob das `why` schon gesetzt ist:
/// [`DiagnosticBuilder::build`] gibt es erst danach. Ein Befund ohne Grund
/// lässt sich also nicht bauen, und niemand muss dafür einen `Result`-Zweig
/// behandeln.
#[derive(Debug, Clone, PartialEq)]
pub struct DiagnosticBuilder<W> {
    code: DiagnosticCode,
    severity: Severity,
    title: String,
    why: W,
    fix: Option<FixAction>,
    docs: Option<String>,
}

impl DiagnosticBuilder<MissingWhy> {
    /// Setzt den Grund und gibt den Bauplan frei.
    ///
    /// Der Text nennt die konkreten Werte dieses Falls: Pfad, Port, Version,
    /// gefundene Fassung. „Konfiguration ungültig" hilft niemandem,
    /// „`hold.timeout_secs = 0` in `/home/x/.config/humanitl/config.toml`,
    /// erlaubt ist 1 bis 3600" schon.
    #[must_use]
    pub fn why(self, why: impl Into<String>) -> DiagnosticBuilder<HasWhy> {
        DiagnosticBuilder {
            code: self.code,
            severity: self.severity,
            title: self.title,
            why: HasWhy(why.into()),
            fix: self.fix,
            docs: self.docs,
        }
    }
}

impl<W> DiagnosticBuilder<W> {
    /// Hängt einen Behebungsvorschlag an.
    #[must_use]
    pub fn fix(mut self, fix: FixAction) -> Self {
        self.fix = Some(fix);
        self
    }

    /// Hängt eine Adresse mit mehr Text an.
    #[must_use]
    pub fn docs(mut self, docs: impl Into<String>) -> Self {
        self.docs = Some(docs.into());
        self
    }

    /// Ändert die Überschrift, die sonst aus dem Register kommt.
    ///
    /// Nur für den seltenen Fall, dass ein Code zwei Ausprägungen hat.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

impl DiagnosticBuilder<HasWhy> {
    /// Fertig.
    #[must_use]
    pub fn build(self) -> Diagnostic {
        Diagnostic {
            code: self.code,
            severity: self.severity,
            title: self.title,
            why: self.why.0,
            fix: self.fix,
            docs: self.docs,
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::codes::{DAEMON_001, SANDBOX_001};
    use super::{Diagnostic, FixAction, Severity};

    #[test]
    fn builder_uses_registry_title() {
        let diagnostic = Diagnostic::builder(SANDBOX_001, Severity::Blocking)
            .why("bwrap not found in PATH=/usr/bin:/bin")
            .fix(FixAction::CopyCommand(
                "sudo apt install bubblewrap".to_owned(),
            ))
            .build();

        assert_eq!(diagnostic.title, "bwrap nicht gefunden");
        assert_eq!(diagnostic.code.as_str(), "SANDBOX_001");
        assert_eq!(diagnostic.severity, Severity::Blocking);
        assert_eq!(diagnostic.why, "bwrap not found in PATH=/usr/bin:/bin");
        assert_eq!(
            diagnostic.fix.as_ref().map(FixAction::as_str),
            Some("copy_command")
        );
        assert!(diagnostic.info().is_some());
    }

    #[test]
    fn display_carries_code_title_and_why() {
        let diagnostic = Diagnostic::builder(DAEMON_001, Severity::Error)
            .why("no socket at /run/user/1000/humanitl/daemon.sock")
            .build();
        assert_eq!(
            diagnostic.to_string(),
            "DAEMON_001: Daemon nicht erreichbar: no socket at /run/user/1000/humanitl/daemon.sock"
        );
    }

    #[test]
    fn severity_is_ordered_from_info_to_blocking() {
        assert!(Severity::Info < Severity::Warning);
        assert!(Severity::Warning < Severity::Error);
        assert!(Severity::Error < Severity::Blocking);
        assert_eq!(Severity::Blocking.as_str(), "blocking");
    }
}
