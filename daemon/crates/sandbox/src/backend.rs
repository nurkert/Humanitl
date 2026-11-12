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
use crate::launcher::{
    CheckResult, IsolationLevel, LaunchPlan, PlanPreview, PlanRequest, SandboxBackend,
};
#[cfg(target_os = "macos")]
use crate::seatbelt::SeatbeltBackend;

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

    fn preview(&self, request: &PlanRequest<'_>) -> PlanPreview {
        dispatch!(self, backend => backend.preview(request))
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

    /// Die Vorschau von bwrap ist die Zeile der festen Vorschau-Nummern mit
    /// Programm, die Tabelle aus derselben Übersetzung, die Umgebung die der
    /// `--setenv`, und die Tür der Proxy-Socket als Datei (HUM-429). Ein
    /// Terminal und die Schicht einer Wegwerf-Sandbox kommen aus den
    /// Optionen, wie beim Start.
    #[test]
    fn the_bwrap_preview_is_the_translated_line_with_its_table() {
        use crate::bwrap_args::LaunchInputs;
        use crate::launcher::{DisposableLayer, Door, LaunchOptions, StdioMode};
        let dir = tempfile::tempdir().expect("tempdir");
        let backend = Backend::fallback(BWRAP_BACKEND, without_bwrap(dir.path())).expect("bwrap");
        let profile = crate::SandboxProfile::parse(
            include_str!("../../../../profiles/sandbox/default.toml"),
            Path::new("default.toml"),
        )
        .expect("the bundled profile");
        let session = crate::SessionContext {
            session: humanitl_core::ids::SessionId::nil(),
            work_src: "/home/u/proj".into(),
            work_mode: humanitl_config::WorkMode::Rw,
            proxy_socket_src: "/run/user/1000/humanitl/proxy/proxy.sock".into(),
            ca_cert_src: "/home/u/.local/share/humanitl/ca/ca.crt".into(),
            ca_bundle_src: "/home/u/.local/share/humanitl/ca/ca-bundle.crt".into(),
            shim_src: "/usr/lib/humanitl/humanitl-shim".into(),
            session_env: Vec::new(),
            command: vec!["true".into()],
            files: Vec::new(),
            agent_program: None,
        };
        let overlay = crate::OverlayDirs {
            upper: "/run/user/1000/humanitl/disposable/LAYER/upper".into(),
            work: "/run/user/1000/humanitl/disposable/LAYER/work".into(),
        };
        for (stdio, disposable) in [
            (StdioMode::Inherit, None),
            (
                StdioMode::Pty { cols: 80, rows: 24 },
                Some(DisposableLayer::Overlay(overlay.clone())),
            ),
        ] {
            let terminal = stdio.pty_size().is_some();
            let options = LaunchOptions {
                stdio,
                disposable: disposable.clone(),
                ..LaunchOptions::default()
            };
            let preview = backend.preview(&PlanRequest::new(&profile, &session, &options));
            let inputs = LaunchInputs::preview_with_agent_files(
                session.files.iter().map(|file| file.dst.clone()),
            )
            .with_controlling_terminal(terminal)
            .with_overlay(disposable.map(|_| overlay.clone()));
            let line = profile.to_bwrap_line(&session, &inputs);
            assert_eq!(preview.argv[0], backend.program().as_os_str());
            assert_eq!(preview.argv[1..], line.args[..]);
            assert_eq!(preview.policy, line.policy);
            assert_eq!(preview.env, profile.effective_env(&session, Some(10)));
            assert!(preview.backend_env.is_empty() && preview.policy_text.is_none());
            for (key, index) in &preview.argv_env {
                assert_eq!(preview.argv[index - 1].to_string_lossy(), key.as_str());
            }
            assert_eq!(
                preview.door,
                Door::SocketFile {
                    host: session.proxy_socket_src.clone(),
                    sandbox: profile.network.proxy_socket_dst.clone(),
                }
            );
            assert_eq!(
                preview.door.describe(),
                "socket_file /run/humanitl/proxy.sock"
            );
            assert!(preview.limits.is_empty());
        }
    }

    /// Zurückgehalten wird nur, wo das Backend einen Wert in der Zeile
    /// nennt, und nur, was der Aufrufer nicht zeigen darf.
    #[test]
    fn withholding_replaces_exactly_the_values_the_caller_hides() {
        use crate::launcher::{Door, PlanPreview, ResourceLimits};
        let mut preview = PlanPreview {
            argv: [
                "bwrap", "--setenv", "A", "1", "--setenv", "B", "2", "--", "B",
            ]
            .into_iter()
            .map(std::ffi::OsString::from)
            .collect(),
            policy: Vec::new(),
            env: vec![("A".into(), "1".into()), ("B".into(), "2".into())],
            backend_env: Vec::new(),
            argv_env: vec![("A".into(), 3), ("B".into(), 6)],
            policy_text: None,
            door: Door::VsockPort {
                host: "/run/x.sock".into(),
                port: 1024,
            },
            limits: ResourceLimits::default(),
        };
        preview.withhold(|key| key == "A", "<withheld>");
        let argv: Vec<String> = preview
            .argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            argv,
            [
                "bwrap",
                "--setenv",
                "A",
                "1",
                "--setenv",
                "B",
                "<withheld>",
                "--",
                "B"
            ]
        );
        assert_eq!(preview.door.describe(), "vsock_port vsock:1024");
        assert_eq!(preview.door.kind(), "vsock_port");
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
