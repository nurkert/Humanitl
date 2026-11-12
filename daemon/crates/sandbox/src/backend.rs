//! Das Backend, das Daemon und Kommandozeile halten (HUM-428).
//!
//! [`Backend`] ist ein Enum über die Backends dieses Builds und reicht jede
//! Methode des Ports [`SandboxBackend`] an das gewählte weiter. Gewählt wird
//! zur Laufzeit, über den Namen aus `sandbox.backend` ([`Backend::detect`]);
//! bis HUM-428 stand an seiner Stelle ein Typ-Alias, der je Build genau ein
//! Backend kannte.
//!
//! Ein Enum und keine `dyn SandboxBackend`-Box: Die Optionen eines Starts
//! tragen einen [`crate::OutputSink`], der Daemon hält das Backend der
//! laufenden Sandbox als Kopie, und ein neues Backend ist eine neue Variante
//! mit ihrem Zweig in jeder Weiterleitung, die der Compiler einfordert.

use std::path::Path;

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, Severity};

use crate::bwrap::BwrapBackend;
use crate::handle::SandboxHandle;
use crate::launcher::{CheckResult, IsolationLevel, LaunchPlan, PlanRequest, SandboxBackend};
#[cfg(target_os = "macos")]
use crate::seatbelt::{SeatbeltBackend, SeatbeltPreview};

/// Der Name von bwrap in `sandbox.backend`.
pub const BWRAP_BACKEND: &str = "bwrap";

/// Der Name von Seatbelt in `sandbox.backend`, nur unter macOS ein Backend.
pub const SEATBELT_BACKEND: &str = "seatbelt";

/// Ruft `$call` auf dem gewählten Backend auf, ein Zweig je Variante.
macro_rules! dispatch {
    ($self:ident, $backend:ident => $call:expr) => {
        match $self {
            Backend::Bwrap($backend) => $call,
            #[cfg(target_os = "macos")]
            Backend::Seatbelt($backend) => $call,
        }
    };
}

/// Ein Sandbox-Backend dieses Builds, zur Laufzeit gewählt.
#[derive(Debug, Clone)]
pub enum Backend {
    /// bubblewrap, unter Linux.
    Bwrap(BwrapBackend),
    /// Seatbelt mit `sandbox-exec`, unter macOS (HUM-323).
    #[cfg(target_os = "macos")]
    Seatbelt(SeatbeltBackend),
}

impl Backend {
    /// Sucht das Backend mit diesem Namen und prüft, ob es auf dieser
    /// Maschine läuft. [`crate::AUTO_BACKEND`] ist das Backend des Systems
    /// ([`crate::PLATFORM_BACKEND_NAME`]).
    ///
    /// # Errors
    ///
    /// Die Befunde des Backends (`SANDBOX_001` bis `SANDBOX_003`), oder
    /// `CONFIG_003` für einen Namen, den dieser Build nicht kennt.
    pub fn detect(name: &str, paths: Paths) -> Result<Self, Diagnostic> {
        match resolve(name)? {
            BWRAP_BACKEND => BwrapBackend::detect(paths).map(Self::Bwrap),
            #[cfg(target_os = "macos")]
            SEATBELT_BACKEND => SeatbeltBackend::detect(paths).map(Self::Seatbelt),
            other => Err(unknown(other)),
        }
    }

    /// Das Backend mit diesem Namen ohne jede Prüfung: der Rückfall, wenn
    /// [`Backend::detect`] scheitert und trotzdem ein Plan gezeigt werden
    /// soll. Ob das Programm da ist und läuft, sagt dann erst der Start, mit
    /// dem Befund des Backends.
    ///
    /// # Errors
    ///
    /// `CONFIG_003` für einen Namen, den dieser Build nicht kennt.
    pub fn fallback(name: &str, paths: Paths) -> Result<Self, Diagnostic> {
        match resolve(name)? {
            BWRAP_BACKEND => Ok(Self::Bwrap(BwrapBackend::fallback(paths))),
            #[cfg(target_os = "macos")]
            SEATBELT_BACKEND => Ok(Self::Seatbelt(SeatbeltBackend::fallback(paths))),
            other => Err(unknown(other)),
        }
    }

    /// Das Programm, das startet: `bwrap` oder `sandbox-exec`.
    #[must_use]
    pub fn program(&self) -> &Path {
        dispatch!(self, backend => backend.program())
    }

    /// Die Pfade, gegen die das Backend plant.
    #[must_use]
    pub const fn paths(&self) -> &Paths {
        match self {
            Self::Bwrap(backend) => backend.paths(),
            #[cfg(target_os = "macos")]
            Self::Seatbelt(backend) => backend.paths(),
        }
    }
}

#[cfg(target_os = "macos")]
impl Backend {
    /// Was eine Sitzung unter Seatbelt täte, ohne Seiteneffekt (HUM-376).
    ///
    /// Übergang bis HUM-429, das die Vorschau in den Port zieht: Unter macOS
    /// zeigen Daemon und Kommandozeile die Vorschau von Seatbelt, dem Backend
    /// des Systems. Hält dieser Wert ein anderes Backend, zeigt sie die
    /// Vorschau von Seatbelt mit dessen Pfaden; [`Backend::detect`] und
    /// [`Backend::fallback`] mit [`crate::AUTO_BACKEND`] liefern unter macOS
    /// immer Seatbelt.
    #[must_use]
    pub fn seatbelt_preview(
        &self,
        request: &PlanRequest<'_>,
        controlling_terminal: bool,
    ) -> SeatbeltPreview {
        match self {
            Self::Seatbelt(backend) => backend.preview(request, controlling_terminal),
            Self::Bwrap(_) => SeatbeltBackend::fallback(self.paths().clone())
                .preview(request, controlling_terminal),
        }
    }
}

impl From<BwrapBackend> for Backend {
    fn from(backend: BwrapBackend) -> Self {
        Self::Bwrap(backend)
    }
}

#[cfg(target_os = "macos")]
impl From<SeatbeltBackend> for Backend {
    fn from(backend: SeatbeltBackend) -> Self {
        Self::Seatbelt(backend)
    }
}

impl SandboxBackend for Backend {
    fn name(&self) -> &'static str {
        dispatch!(self, backend => backend.name())
    }

    fn isolation(&self) -> IsolationLevel {
        dispatch!(self, backend => backend.isolation())
    }

    fn disposable_support(&self, request: &PlanRequest<'_>) -> Result<(), Diagnostic> {
        dispatch!(self, backend => backend.disposable_support(request))
    }

    fn plan(&self, request: &PlanRequest<'_>) -> Result<LaunchPlan, Diagnostic> {
        dispatch!(self, backend => backend.plan(request))
    }

    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        dispatch!(self, backend => backend.launch(plan))
    }

    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult> {
        dispatch!(self, backend => backend.isolation_check(handle))
    }
}

/// Der Name des Backends, mit [`crate::AUTO_BACKEND`] aufgelöst.
fn resolve(name: &str) -> Result<&str, Diagnostic> {
    match name {
        crate::AUTO_BACKEND => Ok(crate::PLATFORM_BACKEND_NAME),
        BWRAP_BACKEND => Ok(BWRAP_BACKEND),
        SEATBELT_BACKEND if cfg!(target_os = "macos") => Ok(SEATBELT_BACKEND),
        other => Err(unknown(other)),
    }
}

/// `CONFIG_003`: ein Name, den dieser Build nicht kennt. Nie ein stiller
/// Rückfall auf ein anderes Backend.
fn unknown(name: &str) -> Diagnostic {
    let platform = match crate::PLATFORM_BACKEND_NAME {
        BWRAP_BACKEND => String::new(),
        other => format!(", \"{other}\""),
    };
    Diagnostic::builder(CONFIG_003, Severity::Blocking)
        .why(format!(
            "sandbox.backend = {name:?}, this build knows \"{}\", \"{BWRAP_BACKEND}\"{platform}",
            crate::AUTO_BACKEND
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::Env;

    use super::*;

    /// Pfade, deren `PATH` auf einen leeren Ordner zeigt: Dort gibt es kein
    /// `bwrap`.
    fn without_bwrap(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.to_string_lossy().into_owned()),
            ("PATH", dir.join("empty-bin").to_string_lossy().into_owned()),
        ]))
    }

    #[test]
    fn bwrap_by_name_on_a_system_without_bwrap_is_sandbox_001() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("empty-bin")).expect("empty bin");
        let err = Backend::detect(BWRAP_BACKEND, without_bwrap(dir.path()))
            .expect_err("no bwrap in PATH");
        assert_eq!(err.code.as_str(), "SANDBOX_001", "{}", err.why);
    }

    #[test]
    fn an_unknown_name_is_refused_and_never_falls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["docker", "", "BWRAP"] {
            let err = Backend::detect(name, without_bwrap(dir.path())).expect_err("unknown");
            assert_eq!(err.code.as_str(), "CONFIG_003", "{name:?}: {}", err.why);
            let err = Backend::fallback(name, without_bwrap(dir.path())).expect_err("unknown");
            assert_eq!(err.code.as_str(), "CONFIG_003", "{name:?}: {}", err.why);
        }
    }

    #[test]
    fn auto_is_the_backend_of_this_system() {
        let dir = tempfile::tempdir().expect("tempdir");
        let auto = Backend::fallback(crate::AUTO_BACKEND, without_bwrap(dir.path()))
            .expect("auto is known");
        assert_eq!(auto.name(), crate::PLATFORM_BACKEND_NAME);
        let bwrap = Backend::fallback(BWRAP_BACKEND, without_bwrap(dir.path())).expect("bwrap");
        assert_eq!(bwrap.name(), BWRAP_BACKEND);
        assert_eq!(bwrap.isolation(), IsolationLevel::Namespaces);
        assert_eq!(bwrap.program(), Path::new("bwrap"));
    }
}
