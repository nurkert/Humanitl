//! Die Unterkommandos und was sie sich teilen.
//!
//! Ein Unterkommando bekommt einen [`Context`] und gibt entweder einen
//! Exit-Code oder einen [`Failure`] zurück. Ein `Failure` ist ein
//! [`Diagnostic`] plus die Zahl, mit der der Prozess endet
//! (`backlog/CONVENTIONS.md` 3.8); die Zuordnung steht an genau einer Stelle,
//! [`exit_code`], damit derselbe Befund überall dieselbe Zahl ergibt.
//!
//! Fachlogik steht hier nicht (ADR-018): `daemon status`, `flows list`,
//! `flows show` und die sieben `rules`-Kommandos sind gRPC-Aufrufe, `config
//! get` und `config schema` lesen die Konfiguration, und die drei
//! `sandbox`-Kommandos rufen `humanitl-sandbox` auf, bis die `Sandbox`-RPC sie
//! ablöst (siehe [`sandbox`]).

pub mod config;
pub mod daemon;
pub mod flows;
pub mod rules;
pub mod run;
pub mod sandbox;

use std::path::{Path, PathBuf};

use humanitl_config::{Env, Paths, ProfileSelection, Resolved};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::{client, diagnostic_from_status};
use tonic::{Code, Status};

use crate::render::Renderer;

/// Alles ging gut.
pub const EXIT_OK: u8 = 0;

/// Ein Fehler des Aufrufers, mit einem Befund, der sagt welcher.
pub const EXIT_USER: u8 = 1;

/// Der Daemon ist nicht erreichbar.
pub const EXIT_DAEMON: u8 = 2;

/// Eine der drei Isolations-Garantien gilt nicht.
pub const EXIT_CHECK: u8 = 3;

/// Eine Sicherheitsverletzung, zum Beispiel ein Authority-Mismatch.
pub const EXIT_SECURITY: u8 = 4;

/// Ein Fehlschlag: der Befund und die Zahl, mit der der Prozess endet.
#[derive(Debug, Clone)]
pub struct Failure {
    /// Was schiefging, mit Grund und wenn möglich Behebung.
    pub diagnostic: Diagnostic,
    /// Der Exit-Code nach `backlog/CONVENTIONS.md` 3.8.
    pub exit: u8,
}

impl Failure {
    /// Ein Fehlschlag mit dem Exit-Code, den sein Code vorgibt.
    #[must_use]
    pub fn new(diagnostic: Diagnostic) -> Self {
        let exit = exit_code(&diagnostic);
        Self { diagnostic, exit }
    }

    /// Ein Fehlschlag mit einem ausdrücklichen Exit-Code.
    ///
    /// Nur, wo die Zahl nicht aus dem Code folgt: `sandbox check` endet mit
    /// [`EXIT_CHECK`], auch wenn der Befund von der Sandbox selbst kommt.
    #[must_use]
    pub const fn with_exit(diagnostic: Diagnostic, exit: u8) -> Self {
        Self { diagnostic, exit }
    }
}

impl From<Diagnostic> for Failure {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::new(diagnostic)
    }
}

/// Der Exit-Code zu einem Befund (`backlog/CONVENTIONS.md` 3.8).
///
/// Nicht erreichbar heißt 2, auch wenn das Token fehlt oder die Proto-Version
/// nicht passt: In allen drei Fällen ist kein brauchbarer Daemon da, und ein
/// Skript, das darauf wartet, will genau das wissen. Eine Garantie, die nicht
/// gilt, heißt 3, ein Authority-Mismatch 4; alles andere ist ein Fehler des
/// Aufrufers und heißt 1.
#[must_use]
pub fn exit_code(diagnostic: &Diagnostic) -> u8 {
    match diagnostic.code.as_str() {
        "DAEMON_001" | "DAEMON_002" | "IPC_001" => EXIT_DAEMON,
        "SANDBOX_004" | "SANDBOX_013" | "SANDBOX_014" | "SANDBOX_015" | "SANDBOX_016" => EXIT_CHECK,
        "PROXY_002" => EXIT_SECURITY,
        _ => EXIT_USER,
    }
}

/// Was jedes Unterkommando kennt.
#[derive(Debug)]
pub struct Context {
    /// Die Umgebung, genau einmal gelesen.
    pub env: Env,
    /// Die Pfade nach XDG.
    pub paths: Paths,
    /// Das Arbeitsverzeichnis.
    pub cwd: PathBuf,
    /// Die Konfigurations-Flags der Kommandozeile als `(Pfad, Text)`.
    pub cli_config: Vec<(String, String)>,
    /// Eine ausdrücklich genannte `config.toml` (`--config`).
    pub config_file: Option<PathBuf>,
    /// Der Name aus `--profile`, falls einer dasteht.
    pub profile: Option<String>,
    /// Was `--profile` in diesem Aufruf benennt.
    pub profile_means: ProfileMeaning,
    /// Wie ausgegeben wird.
    pub render: Renderer,
}

/// Was `--profile` benennt.
///
/// `backlog/CONVENTIONS.md` 3.8 führt `--profile NAME` für `humanitl run`
/// (Sitzungsprofil aus HUM-066) und für `humanitl sandbox run|argv`
/// (bwrap-Profil unter `profiles/sandbox/`). Welches gemeint ist, entscheidet
/// das Unterkommando und sonst nichts — insbesondere nicht, welche Dateien
/// gerade auf der Platte liegen: Eine Bedeutung, die davon abhinge, kippte
/// lautlos, sobald jemand ein gleichnamiges Profil anlegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMeaning {
    /// Das Profil der Sitzung (HUM-066). Der Regelfall.
    Session,
    /// Das bwrap-Profil unter `profiles/sandbox/`. Nur unter `humanitl sandbox`.
    Sandbox,
}

impl Context {
    /// Baut den Kontext aus der Umgebung des Prozesses.
    #[must_use]
    pub fn new(
        cli_config: Vec<(String, String)>,
        config_file: Option<PathBuf>,
        profile: Option<String>,
        profile_means: ProfileMeaning,
        render: Renderer,
    ) -> Self {
        let env = Env::from_process();
        let paths = Paths::new(env.clone());
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            env,
            paths,
            cwd,
            cli_config,
            config_file,
            profile,
            profile_means,
            render,
        }
    }

    /// Der Name aus `--profile`, falls einer dasteht.
    #[must_use]
    pub fn profile_flag(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// Die Profilauswahl der Kommandozeile.
    ///
    /// Leer unter `humanitl sandbox`: Dort benennt `--profile` das bwrap-Profil.
    #[must_use]
    pub fn selection(&self) -> ProfileSelection {
        ProfileSelection {
            name: match self.profile_means {
                ProfileMeaning::Session => self.profile.clone(),
                ProfileMeaning::Sandbox => None,
            },
        }
    }

    /// Die Konfigurations-Flags, unter `humanitl sandbox` um `sandbox.profile`
    /// ergänzt.
    #[must_use]
    pub fn cli_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = self.cli_config.clone();
        if let (ProfileMeaning::Sandbox, Some(name)) = (self.profile_means, self.profile_flag())
            && !pairs.iter().any(|(key, _)| key == "sandbox.profile")
        {
            pairs.push(("sandbox.profile".to_owned(), name.to_owned()));
        }
        pairs
    }

    /// Lädt die Konfiguration und meldet, was das Laden überlebt hat.
    ///
    /// Streng: Ein `--profile`, das ein Sitzungsprofil benennt, muss eines
    /// benennen, das es gibt. Ein Tippfehler ist `CONFIG_001` und kein stiller
    /// Start mit dem Vorgabeprofil; ein Profil, das da ist, sich aber nicht
    /// lesen lässt, ebenso.
    ///
    /// # Errors
    ///
    /// `CONFIG_001`, `CONFIG_002` oder `CONFIG_003`, wenn eine Datei, ein
    /// Profil oder ein Flag nicht stimmt.
    pub fn config(&self) -> Result<Resolved, Failure> {
        let mut sources = humanitl_config::sources_for(
            &self.selection(),
            Some(&self.cwd),
            &self.env,
            &self.cli_pairs(),
        )
        .map_err(Failure::new)?;
        if let Some(file) = self.config_file.as_ref() {
            sources.global_toml = Some(file.clone());
        }
        Ok(self.report(humanitl_config::load(&sources).map_err(Failure::new)?))
    }

    /// Schreibt die Befunde einer Auflösung und gibt sie weiter.
    fn report(&self, resolved: Resolved) -> Resolved {
        for diagnostic in &resolved.diagnostics {
            self.render
                .note(&crate::render::diagnostic_block(diagnostic));
        }
        if let Some(diagnostic) = self.paths.runtime_dir().diagnostic {
            self.render
                .detail(&crate::render::diagnostic_block(&diagnostic));
        }
        resolved
    }

    /// Verbindet sich mit dem Daemon.
    ///
    /// # Errors
    ///
    /// `DAEMON_001`, wenn Token oder Socket fehlen, mit Exit-Code
    /// [`EXIT_DAEMON`].
    pub async fn connect(&self) -> Result<Client, Failure> {
        client::connect(&self.paths).await.map_err(Failure::new)
    }
}

/// Ein `Status` vom Daemon als Befund.
///
/// Trägt der `Status` einen Befund in seinen Details, gilt der: er kommt aus
/// dem Register und weiß mehr als der gRPC-Code. Sonst wird der Code
/// übersetzt; `what` benennt den Aufruf, damit im `why` steht, was
/// fehlgeschlagen ist.
///
/// Nur `Unavailable` heißt „Daemon nicht erreichbar" (`DAEMON_001`, Exit 2),
/// und `Unauthenticated` heißt „kein brauchbarer Daemon" (`IPC_001`, ebenfalls
/// Exit 2). Jede andere Antwort kommt von einem laufenden Daemon und ist ein
/// Fehler des Aufrufs: `CLI_001` mit Exit 1 (`backlog/CONVENTIONS.md` 3.8).
#[must_use]
pub fn status_diagnostic(status: &Status, what: &str) -> Diagnostic {
    if let Some(diagnostic) = diagnostic_from_status(status)
        && let Some(rebuilt) = from_proto(&diagnostic)
    {
        return rebuilt;
    }
    match status.code() {
        Code::Unauthenticated => Diagnostic::builder(codes::IPC_001, Severity::Blocking)
            .why(format!(
                "{what} was refused: {}; the session token does not match the running daemon",
                status.message()
            ))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build(),
        Code::Unavailable => Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(format!("{what} failed: {}", status.message()))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build(),
        // Der Daemon hat geantwortet, also ist er erreichbar. Ein
        // `InvalidArgument` oder `FailedPrecondition` ist ein Fehler des
        // Aufrufs, kein fehlender Dienst: `DAEMON_001` und damit
        // [`EXIT_DAEMON`] wäre hier eine falsche Auskunft an jedes Skript, das
        // auf den Daemon wartet. `DAEMON_001` bleibt `Unavailable` und dem
        // Verbindungsaufbau vorbehalten.
        other => Diagnostic::builder(codes::CLI_001, Severity::Error)
            .why(format!(
                "{what} failed with {}: {}",
                other.description(),
                status.message()
            ))
            .build(),
    }
}

/// Baut aus dem Befund der Wire-Form wieder einen [`Diagnostic`].
///
/// Nur der Code, die Stufe, der Grund und die Adresse reisen zurück; die
/// Überschrift kommt aus dem Register des Clients, damit sie in der Sprache
/// dieser Installation steht. Ein Code, den das Register nicht kennt, ergibt
/// `None`: dann ist der Daemon neuer als der Client, und der `Status` selbst
/// sagt mehr als ein erfundener Befund.
#[must_use]
pub fn from_proto(diagnostic: &humanitl_ipc::v1::Diagnostic) -> Option<Diagnostic> {
    let info = humanitl_core::diagnostics::lookup_str(&diagnostic.code)?;
    let severity = match diagnostic.severity {
        1 => Severity::Info,
        2 => Severity::Warning,
        4 => Severity::Blocking,
        _ => Severity::Error,
    };
    let mut builder = Diagnostic::builder(info.code, severity).why(diagnostic.why.clone());
    if !diagnostic.docs_url.is_empty() {
        builder = builder.docs(diagnostic.docs_url.clone());
    }
    if let Some(fix) = diagnostic.fix.as_ref().and_then(fix_from_proto) {
        builder = builder.fix(fix);
    }
    Some(builder.build())
}

/// Der Behebungsvorschlag aus der Wire-Form, soweit er ohne Regel auskommt.
///
/// `add_rule` bleibt aus: der Vorschlag trägt eine ganze Regel, und die gehört
/// in eine Zeile, die man abtippen kann. Sie hier halb zu übersetzen wäre
/// schlechter, als sie wegzulassen; wer die Regel sehen will, liest sie mit
/// `--json` aus dem Befund.
fn fix_from_proto(fix: &humanitl_ipc::v1::FixAction) -> Option<FixAction> {
    use humanitl_ipc::v1::fix_action::Action;

    match fix.action.as_ref()? {
        Action::SetEnv(set) => Some(FixAction::SetEnv {
            key: set.key.clone(),
            value: set.value.clone(),
        }),
        Action::InstallService(()) => Some(FixAction::InstallService),
        Action::ChangeSetting(change) => Some(FixAction::ChangeSetting {
            key: change.key.clone(),
            value: change.value.clone(),
        }),
        Action::CopyCommand(command) => Some(FixAction::CopyCommand(command.clone())),
        Action::OpenUrl(url) => Some(FixAction::OpenUrl(url.clone())),
        Action::RemountReadOnly(path) => Some(FixAction::RemountReadOnly(PathBuf::from(path))),
        Action::AddRule(_) => None,
    }
}

/// Die Meldung für ein Unterkommando, das es noch nicht gibt.
///
/// Sie nennt das Issue, damit klar ist, worauf man wartet.
#[must_use]
pub fn not_yet(what: &str, arrives: &str) -> String {
    format!("{what} arrives in {arrives}")
}

/// Ein Unterkommando, das der Vertrag kennt und dieses Binary noch nicht.
///
/// Ein [`Failure`] wie jeder andere, damit der Aufrufer denselben Weg
/// bekommt wie bei jedem Fehlschlag: mit `--json` eine Zeile JSON auf
/// `stdout`, sonst den Block auf `stderr`. Ein nackter Satz auf `stderr` wäre
/// für ein Skript unsichtbar gewesen. `why` nennt das Kommando, `fix` das
/// Issue, das es bringt; der Exit-Code ist [`EXIT_USER`], denn getan hat der
/// Aufruf nichts.
#[must_use]
pub fn not_yet_failure(what: &str, arrives: &str) -> Failure {
    Failure::with_exit(
        Diagnostic::builder(codes::CLI_003, Severity::Error)
            .why(not_yet(what, arrives))
            .fix(FixAction::OpenUrl(format!(
                "{}/issues?q={arrives}",
                env!("CARGO_PKG_REPOSITORY")
            )))
            .build(),
        EXIT_USER,
    )
}

/// Prüft, ob ein Pfad auf eine ausführbare Datei zeigt.
#[must_use]
pub fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes::{
        CONFIG_002, DAEMON_001, IPC_001, PROXY_002, SANDBOX_011, SANDBOX_014,
    };
    use humanitl_core::{Diagnostic, Severity};
    use humanitl_ipc::diagnostic_to_proto;
    use tonic::{Code, Status};

    use super::{
        EXIT_CHECK, EXIT_DAEMON, EXIT_SECURITY, EXIT_USER, exit_code, from_proto, status_diagnostic,
    };

    fn diagnostic(code: humanitl_core::DiagnosticCode) -> Diagnostic {
        Diagnostic::builder(code, Severity::Blocking)
            .why("because")
            .build()
    }

    #[test]
    fn every_exit_code_of_conventions_38_has_a_diagnostic_that_reaches_it() {
        assert_eq!(exit_code(&diagnostic(DAEMON_001)), EXIT_DAEMON);
        assert_eq!(exit_code(&diagnostic(IPC_001)), EXIT_DAEMON);
        assert_eq!(exit_code(&diagnostic(SANDBOX_014)), EXIT_CHECK);
        assert_eq!(exit_code(&diagnostic(PROXY_002)), EXIT_SECURITY);
        assert_eq!(exit_code(&diagnostic(CONFIG_002)), EXIT_USER);
        assert_eq!(exit_code(&diagnostic(SANDBOX_011)), EXIT_USER);
    }

    #[test]
    fn a_status_without_details_is_translated_from_its_code() {
        let refused = status_diagnostic(&Status::unauthenticated("no token"), "GetInfo");
        assert_eq!(refused.code.as_str(), "IPC_001");
        assert!(refused.why.contains("GetInfo"));

        let gone = status_diagnostic(&Status::unavailable("socket closed"), "ListFlows");
        assert_eq!(gone.code.as_str(), "DAEMON_001");
        assert_eq!(exit_code(&gone), EXIT_DAEMON);

        let other = status_diagnostic(&Status::internal("boom"), "GetFlow");
        assert_eq!(other.code.as_str(), "CLI_001");
        assert_eq!(other.title, "Aufruf am Daemon abgelehnt");
    }

    #[test]
    fn a_daemon_that_answers_is_never_a_daemon_that_is_missing() {
        // Der Daemon läuft und hat geantwortet; nur der Aufruf passt nicht.
        // Exit 2 hieße „Daemon nicht erreichbar" und wäre gelogen.
        for status in [
            Status::failed_precondition("no session is running"),
            Status::invalid_argument("hold.timeout_secs is not a number"),
            Status::unimplemented("Sandbox arrives in HUM-040"),
            Status::not_found("no flow with that id"),
        ] {
            let diagnostic = status_diagnostic(&status, "Sandbox");
            assert_eq!(diagnostic.code.as_str(), "CLI_001", "{status:?}");
            assert_eq!(exit_code(&diagnostic), EXIT_USER, "{status:?}");
        }

        // Und umgekehrt: nur diese beiden bleiben bei Exit 2.
        for status in [
            Status::unavailable("socket closed"),
            Status::unauthenticated("no token"),
        ] {
            assert_eq!(
                exit_code(&status_diagnostic(&status, "GetInfo")),
                EXIT_DAEMON,
                "{status:?}"
            );
        }
    }

    #[test]
    fn a_missing_subcommand_is_a_diagnostic_with_its_issue() {
        let failure = super::not_yet_failure("humanitl run", "HUM-067");
        assert_eq!(failure.exit, EXIT_USER);
        assert_eq!(failure.diagnostic.code.as_str(), "CLI_003");
        assert!(failure.diagnostic.why.contains("humanitl run"));
        assert!(failure.diagnostic.why.contains("HUM-067"));
        let fix = crate::render::fix_line(failure.diagnostic.fix.as_ref().expect("a fix"));
        assert!(fix.contains("HUM-067"), "{fix}");
    }

    #[test]
    fn a_status_with_details_keeps_the_code_of_the_daemon() {
        let original = Diagnostic::builder(CONFIG_002, Severity::Warning)
            .why("hold.timeuot_secs is not a key")
            .build();
        let status = humanitl_ipc::diagnostic_to_status(&original);
        assert_eq!(status.code(), Code::InvalidArgument);

        let back = status_diagnostic(&status, "SetConfig");
        assert_eq!(back.code.as_str(), "CONFIG_002");
        assert_eq!(back.why, original.why);
        assert_eq!(back.severity, Severity::Warning);
    }

    #[test]
    fn an_unknown_code_from_the_wire_is_not_invented() {
        let mut proto = diagnostic_to_proto(&diagnostic(DAEMON_001));
        proto.code = "FUTURE_042".to_owned();
        assert!(from_proto(&proto).is_none());
    }
}
