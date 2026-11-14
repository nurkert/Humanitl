//! Der Port [`SandboxBackend`] und sein Vokabular (CONVENTIONS.md 3.4).
//!
//! Ein Backend macht aus einer [`PlanRequest`] (Profil, Sitzung und
//! [`LaunchOptions`]) einen [`LaunchPlan`], startet ihn zu einem
//! [`SandboxHandle`] und liest aus der laufenden Sandbox die drei Garantien
//! als [`CheckResult`]. Daemon und Kommandozeile halten das Backend als
//! [`crate::Backend`], ein Enum über die Backends dieses Builds, gewählt zur
//! Laufzeit (HUM-428); was ein Start außer Profil und Sitzung braucht, steht
//! in den [`LaunchOptions`] und nicht mehr in Methoden eines konkreten Typs.
//! Was eine Sitzung täte, ohne etwas anzulegen, sagt
//! [`SandboxBackend::preview`] als [`PlanPreview`]: Zeile, Umgebung, Tabelle
//! der Pfade ([`PolicyRow`]) und die eine Tür ([`Door`]). Das Backend baut
//! die Tabelle aus denselben Daten wie seine Zeile; kein Aufrufer liest dafür
//! die Flags eines Backends (HUM-429). Dass die Kopplung an bwrap nicht
//! weiter wächst, prüft `tools/check_coupling.py` (HUM-145).
//!
//! Der Plan ist vollständig: `argv[0]` ist das Programm, alles danach kommt
//! vom Backend, und die Oberfläche zeigt die Liste wörtlich
//! ([`LaunchPlan::argv_line`]). Ein Plan startet genau eine Sandbox: er trägt
//! die Deskriptoren, die das Programm erbt, und die Leseseiten der Pipes, über
//! die die Sandbox zurückmeldet; beides ist beim zweiten Start nicht mehr da.

use std::ffi::OsString;
use std::io::PipeReader;
use std::os::fd::{OwnedFd, RawFd};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use humanitl_core::Diagnostic;
use humanitl_core::ids::SessionId;

use crate::bwrap_args::shell_line;
use crate::disposable::OverlayDirs;
use crate::handle::{OutputSink, SandboxHandle};
use crate::profile::{SandboxProfile, SessionContext};

/// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des Shims
/// wartet, wenn [`LaunchOptions::report_timeout`] nichts anderes sagt.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(5);

/// Ein Sandbox-Backend: plant, startet, prüft.
pub trait SandboxBackend: Send + Sync {
    /// Der Name des Backends, `"bwrap"` oder `"seatbelt"`.
    fn name(&self) -> &'static str;

    /// Wie stark dieses Backend die Sandbox vom Host trennt.
    fn isolation(&self) -> IsolationLevel;

    /// Ob diese Sitzung eine Wegwerf-Sandbox bekommen kann (HUM-357):
    /// dieselben Bedingungen, die der Start einer Wegwerf-Sandbox prüft, ohne
    /// etwas anzulegen. Die Auflösung von `sandbox.mode = "auto"` fragt sie
    /// vor dem Start ([`crate::resolve_mode`]).
    ///
    /// # Errors
    ///
    /// `SANDBOX_030` mit dem ersten fehlenden Teil und seinem Fix.
    fn disposable_support(&self, request: &PlanRequest<'_>) -> Result<(), Diagnostic>;

    /// Was eine Sitzung mit dieser Anfrage täte, ohne Seiteneffekt (HUM-429):
    /// kein Deskriptor, kein Verzeichnis, kein Port. Was erst der Start
    /// festlegt, steht als Platzhalter da. Anders als
    /// [`SandboxBackend::plan`] scheitert die Vorschau nicht; wer nachsehen
    /// will, braucht weder Proxy noch Shim noch Projektordner.
    ///
    /// Ob die Sitzung ein Terminal bekommt, sagt [`LaunchOptions::stdio`]
    /// wie beim Start.
    fn preview(&self, request: &PlanRequest<'_>) -> PlanPreview;

    /// Baut aus der Anfrage die vollständige Kommandozeile samt allem, was
    /// der Start braucht, und prüft dabei jede Vorbedingung, die sich ohne
    /// Start prüfen lässt.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit registriertem Code; welche, sagt das Backend.
    fn plan(&self, request: &PlanRequest<'_>) -> Result<LaunchPlan, Diagnostic>;

    /// Startet den Plan. Kehrt zurück, sobald die Sandbox läuft oder sicher
    /// nicht läuft.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`], wenn das Programm nicht startet oder endet, bevor
    /// der Befehl in der Sandbox lief.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic>;

    /// Liest aus der laufenden Sandbox, ob die drei Garantien gelten.
    ///
    /// Ein Ergebnis je [`IsolationCheck`], in der Reihenfolge der Varianten.
    /// Gewartet wird so lange, wie [`LaunchOptions::report_timeout`] beim
    /// Planen sagte: Die Frist reist mit dem Plan in das Handle, damit jede
    /// Prüfung unter den Bedingungen des Starts läuft, auch mit einer
    /// anderen Instanz des Backends.
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult>;

    /// Zieht ein Abbild als Wurzel der Sandbox, auf ausdrücklichen Wunsch
    /// (HUM-439): `reference`, oder leer das eingestellte. Das Ziehen nutzt
    /// das Netz direkt, nicht durch den Proxy; kein Start tut es je.
    ///
    /// Ein Backend, das hier ankommt, ist schon gebaut, also zugelassen oder
    /// über die Umgehung der Escape-Suite gekommen; vor der Zulassung lehnt
    /// `Backend::detect` mit `SANDBOX_048` ab, und dann zieht nichts. Die
    /// Vorgabe gilt für jedes Backend, das auf den Dateien des Rechners
    /// läuft und keine Abbilder kennt.
    ///
    /// Abbrechen lässt sich das Ziehen über `cancel` (HUM-457): Wer zieht,
    /// fragt das Zeichen regelmäßig ab, beendet bei gesetztem Zeichen den
    /// Prozess der Engine samt allen Prozessen, die er gestartet hat, und
    /// kehrt mit einem Befund zurück. Das Zeichen ist ein Wert des Aufrufers
    /// und kein Verzeichnis im Backend: Welcher Abbruch zu welchem Ziehen
    /// gehört, entscheidet der Aufrufer über seine Kennung, und ein Backend
    /// ohne Abbilder braucht nichts davon.
    ///
    /// # Errors
    ///
    /// Die Vorgabe: `SANDBOX_034`. Ein Backend mit Abbildern nennt seine
    /// eigenen Befunde (`CONFIG_003`, `SANDBOX_043`, auch für ein
    /// abgebrochenes Ziehen).
    fn pull_image(&self, reference: &str, cancel: &PullCancel) -> Result<PulledImage, Diagnostic> {
        let _ = (reference, cancel);
        Err(without_images(self.name()))
    }
}

/// Wie stark ein Backend die Sandbox vom Host trennt, geordnet: Eine
/// Untergrenze vergleicht mit `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IsolationLevel {
    /// Eine Richtlinie des Kerns über einem Prozess, der die echten Pfade
    /// sieht (Seatbelt unter macOS).
    Policy,
    /// Eigene Namensräume über dem geteilten Kern des Hosts (bwrap).
    Namespaces,
    /// Ein eigener Gastkern in einer Micro-VM (das VM-Backend, HUM-464).
    Kernel,
}

impl IsolationLevel {
    /// Die drei, von schwach nach stark.
    pub const ALL: [Self; 3] = [Self::Policy, Self::Namespaces, Self::Kernel];

    /// Der Name in `snake_case`, wie ihn Konfiguration, Protokoll und
    /// Oberfläche führen. Stabil: Er wird ein Wert von
    /// `sandbox.min_isolation` (HUM-431).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Policy => "policy",
            Self::Namespaces => "namespaces",
            Self::Kernel => "kernel",
        }
    }
}

/// Die Untergrenze aus `sandbox.min_isolation` als Stufe (HUM-431). Beide
/// Enums führen dieselben drei Namen in derselben Ordnung; der Test
/// `the_isolation_of_a_name_is_that_of_its_backend` in `backend.rs` hält das fest.
impl From<humanitl_config::MinIsolation> for IsolationLevel {
    fn from(floor: humanitl_config::MinIsolation) -> Self {
        match floor {
            humanitl_config::MinIsolation::Policy => Self::Policy,
            humanitl_config::MinIsolation::Namespaces => Self::Namespaces,
            humanitl_config::MinIsolation::Kernel => Self::Kernel,
        }
    }
}

/// Die Schicht einer Wegwerf-Sandbox, in der Form, die das Backend nimmt
/// (HUM-350, HUM-374).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisposableLayer {
    /// Obere Schicht und Arbeitsverzeichnis eines Overlays über dem Projekt
    /// (bwrap).
    Overlay(OverlayDirs),
    /// Ein Klon des Projekts, der an dessen Stelle das Projekt der Sandbox
    /// ist (Seatbelt).
    Clone(PathBuf),
}

/// Was ein Start außer Profil und Sitzung braucht (HUM-428).
///
/// Bis HUM-428 waren das Builder-Methoden der konkreten Backends, und ein
/// Aufrufer musste deshalb den Typ kennen. Jetzt ist es ein Wert, den jedes
/// Backend gleich bekommt. Eine Wegwerf-Schicht in einer Form, die es nicht
/// nimmt, lehnt ein Backend ab, statt sie still zu übergehen.
#[derive(Debug, Clone, Default)]
pub struct LaunchOptions {
    /// Wohin Ein- und Ausgabe der Sandbox gehen; Standard
    /// [`StdioMode::Inherit`].
    pub stdio: StdioMode,
    /// Wohin jedes gelesene Stück Ausgabe zusätzlich geht, während die Sandbox
    /// läuft.
    ///
    /// Wirkt nur, wo es Ausgabe zu lesen gibt ([`StdioMode::Capture`],
    /// [`StdioMode::Pty`]). Der Sender steht vor dem Start fest und nicht
    /// danach, damit kein Byte zwischen Start und Anmeldung verloren geht.
    /// Wer auf das Ende des Kanals wartet, gibt ihn nur den Optionen des
    /// Starts und keiner Kopie, die gehalten wird: Ein gehaltener Sender
    /// schlösse den Kanal nie.
    pub output_sink: Option<OutputSink>,
    /// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des
    /// Shims wartet; ohne Angabe [`REPORT_TIMEOUT`].
    pub report_timeout: Option<Duration>,
    /// Der aufgelöste Ort, an dem der Aufrufer den Projektordner geprüft und
    /// freigegeben hat (HUM-294).
    ///
    /// Der Plan öffnet den Projektordner einmal (HUM-263) und nimmt ihn nur,
    /// wenn der Ort des Geöffneten genau dieser ist. Der Aufrufer prüft mehr
    /// als die Politik des Backends, etwa ob der Ordner unter dem
    /// Heimatverzeichnis liegt oder erklärt ist; ersetzt ein Verweis zwischen
    /// seiner Prüfung und dem Öffnen einen Teil des Pfades, führte derselbe
    /// Text sonst an einen Ort, den nur die Denylist gesehen hat. Ohne Angabe
    /// gilt allein die Politik.
    pub approved_work_dir: Option<PathBuf>,
    /// Gerät und Inode des Projektordners, den der Aufrufer schon geöffnet
    /// hält (HUM-350): Der Plan bindet den Ordner nur, wenn der dort
    /// geöffnete dieselben trägt. Der Aufrufer schreibt später durch seinen
    /// Deskriptor; ein Umbenennen und ein neuer Ordner unter demselben Pfad
    /// dazwischen hängte sonst einen anderen Ordner ein als den, in den
    /// übernommen wird. Seatbelt hängt nichts ein und liest das Feld nicht;
    /// dort ist der Klon der Ordner der Sandbox (HUM-374).
    pub expected_work_inode: Option<(u64, u64)>,
    /// Die Schicht, wenn dies eine Wegwerf-Sandbox ist. Ob ein Backend eine
    /// kann, sagt [`SandboxBackend::disposable_support`].
    pub disposable: Option<DisposableLayer>,
    /// Nur lesbare Binds, die der Aufrufer zur Sitzung beiträgt und nicht das
    /// Profil, als `(Quelle, Ziel)`: das Testverzeichnis der Escape-Tests
    /// (`--tests-dir`).
    ///
    /// Sie gehen denselben Weg wie die Einträge aus `mounts.extra_ro`
    /// (HUM-263): Der Plan öffnet die Quelle einmal, prüft den Ort ihres
    /// Deskriptors mit [`crate::MountPolicy::check_session_dir`] und bindet
    /// den Deskriptor, hinter den Erweiterungen des Profils und vor dem
    /// Projektordner. Geprüft wird wie beim Projektordner, denn auch diese
    /// Quelle kommt aus dem Aufruf und darf unter dem Heimatverzeichnis
    /// liegen; das Heimatverzeichnis selbst und die geschützten Bäume nicht.
    pub session_ro: Vec<(PathBuf, PathBuf)>,
    /// Wie das Terminal einer Sitzung am eigenen Pseudoterminal reist
    /// ([`StdioMode::Pty`]); ohne Pseudoterminal ohne Bedeutung (HUM-443).
    pub terminal: TerminalTransport,
}

/// Wie das Terminal des Agenten und jeder weiteren Shell zwischen Daemon und
/// Sandbox reist (HUM-443).
///
/// Die Kernel-Stufe kennt nur den Strom: In einer VM überquert kein
/// Deskriptor die Grenze. Unter bwrap gibt es ihn, damit beide Modi dieselben
/// Terminal-Tests bestehen; ein Backend, das ihn nicht trägt, lehnt ihn ab,
/// statt still den Deskriptor zu nehmen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalTransport {
    /// Die Herrscherseite beim Daemon, die Untertanenseite als Deskriptor in
    /// der Sandbox; weitere Shells über die Tür mit `SCM_RIGHTS` (HUM-299).
    #[default]
    Descriptor,
    /// Die Pseudoterminals im Shim, Bytes und Steuerung als Rahmen auf einem
    /// Strom-Socket (`--control-stream-fd`, [`crate::control_frame`]).
    Stream,
}

/// Ein Backend, das den Strom-Modus nicht trägt, lehnt ihn ab (HUM-443),
/// statt still den Deskriptor zu nehmen.
///
/// # Errors
///
/// `TERM_002`, wenn `options` den Strom an einem Pseudoterminal verlangt.
pub(crate) fn refuse_stream(backend: &str, options: &LaunchOptions) -> Result<(), Diagnostic> {
    if options.terminal == TerminalTransport::Stream && options.stdio.pty_size().is_some() {
        return Err(Diagnostic::builder(
            humanitl_core::diagnostics::codes::TERM_002,
            humanitl_core::Severity::Blocking,
        )
        .why(format!(
            "the backend {backend} carries the terminal only as a descriptor, and this start asks \
             for the control stream"
        ))
        .build());
    }
    Ok(())
}

impl LaunchOptions {
    /// Die Frist für den Bericht des Shims, mit der Vorgabe
    /// [`REPORT_TIMEOUT`].
    #[must_use]
    pub fn effective_report_timeout(&self) -> Duration {
        self.report_timeout.unwrap_or(REPORT_TIMEOUT)
    }

    /// Die Schicht eines Overlays, wenn eine gewählt ist.
    #[must_use]
    pub fn overlay(&self) -> Option<&OverlayDirs> {
        match &self.disposable {
            Some(DisposableLayer::Overlay(dirs)) => Some(dirs),
            Some(DisposableLayer::Clone(_)) | None => None,
        }
    }
}

/// Was ein Backend zum Planen bekommt.
#[derive(Debug, Clone, Copy)]
pub struct PlanRequest<'a> {
    /// Das geprüfte Profil.
    pub profile: &'a SandboxProfile,
    /// Die Sitzung, für die geplant wird.
    pub session: &'a SessionContext,
    /// Was der Start außerdem braucht.
    pub options: &'a LaunchOptions,
}

impl<'a> PlanRequest<'a> {
    /// Eine Anfrage aus ihren drei Teilen.
    #[must_use]
    pub const fn new(
        profile: &'a SandboxProfile,
        session: &'a SessionContext,
        options: &'a LaunchOptions,
    ) -> Self {
        Self {
            profile,
            session,
            options,
        }
    }
}

/// Ein Abbild, das [`SandboxBackend::pull_image`] in den lokalen Speicher
/// der Container-Engine geholt und danach mit seinem Digest gefunden hat
/// (HUM-439).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PulledImage {
    /// Der Verweis, wie er gezogen wurde: `name@sha256:<hex>`.
    pub reference: String,
    /// Der Digest, `sha256:<hex>`.
    pub digest: String,
    /// Die Kennung des Abbilds in der Engine.
    pub id: String,
}

/// Das Zeichen, mit dem ein laufendes Ziehen abgebrochen wird (HUM-457).
///
/// Gesetzt wird es einmal und nie zurück; wer zieht, liest es zwischen zwei
/// Blicken auf den Prozess der Engine. Das Beenden selbst macht das Backend,
/// das den Prozess kennt, und es trägt hier ein, ob es die Engine wirklich
/// unterbrochen hat und wie viele Prozesse des Ziehens danach noch liefen
/// ([`PullCancel::interrupted`]). Nur das sagt dem Aufrufer, ob sein Abbruch
/// gewirkt hat: Ein Ziehen, das kurz vor dem Zeichen von selbst fertig
/// wurde, hat niemand abgebrochen.
#[derive(Debug, Default)]
pub struct PullCancel {
    requested: std::sync::atomic::AtomicBool,
    interrupted: Mutex<Option<usize>>,
}

impl PullCancel {
    /// Ein Zeichen, das noch nicht gesetzt ist.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Bittet um den Abbruch. Wirkt einmal; ein zweiter Aufruf ändert nichts.
    pub fn cancel(&self) {
        self.requested
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Ob um den Abbruch gebeten wurde.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Trägt ein, dass der Abbruch die Engine unterbrochen hat; `still_running`
    /// ist die Zahl der Prozesse des Ziehens, die danach noch liefen. Nur das
    /// Backend ruft das, und nur, wenn es wirklich unterbrochen hat.
    pub fn mark_interrupted(&self, still_running: usize) {
        *self
            .interrupted
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(still_running);
    }

    /// `Some(n)`: Das Backend hat die Engine unterbrochen, und danach liefen
    /// noch `n` Prozesse des Ziehens. `None`: Unterbrochen wurde nichts, das
    /// Ziehen endete von selbst oder vor dem Start der Engine.
    #[must_use]
    pub fn interrupted(&self) -> Option<usize> {
        *self
            .interrupted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// `SANDBOX_034`: Dieses Backend läuft auf den Dateien des Rechners und kennt
/// keine Abbilder (HUM-439).
fn without_images(name: &str) -> Diagnostic {
    use humanitl_core::diagnostics::codes::SANDBOX_034;
    use humanitl_core::{FixAction, Severity};

    Diagnostic::builder(SANDBOX_034, Severity::Blocking)
        .why(format!(
            "the backend {name:?} runs on the host's own files and has no images; only the \
             container engine backend takes an image as the root of the sandbox"
        ))
        .fix(FixAction::CopyCommand(
            "humanitl sandbox backends".to_owned(),
        ))
        .build()
}

/// Was eine Zeile der Tabelle mit ihrem Pfad tut (HUM-429).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyMode {
    /// Lesbar, von der Quelle auf dem Host.
    ReadOnly,
    /// Les- und schreibbar, von der Quelle auf dem Host.
    ReadWrite,
    /// Weder les- noch schreibbar (Seatbelt: eine Sperre im Profil).
    Denied,
    /// Eine Datei aus dem Speicher des Launchers über dem Pfad: eine Maske,
    /// eine Identitätsdatei, eine Datei des Adapters.
    Masked,
    /// Ein leeres Dateisystem im Speicher.
    Tmpfs,
    /// Ein Verweis, sein Ziel in [`PolicyRow::link`].
    Symlink,
    /// Das `/proc` der Sandbox.
    Proc,
    /// Das minimale `/dev` der Sandbox.
    Dev,
    /// Die Wurzel der Sandbox ist ein Abbild der Container-Engine, nicht
    /// das Gerüst des Profils (HUM-439). [`PolicyRow::source`] trägt den
    /// Verweis auf das Abbild samt Digest (`name@sha256:...`), keinen Pfad
    /// auf dem Rechner.
    Image,
}

/// Woher eine Zeile der Tabelle stammt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyOrigin {
    /// Aus dem Profil oder den festen Regeln des Backends.
    Profile,
    /// Aus der Sitzung: Projekt, Proxy-Socket, CA, Shim.
    Session,
    /// Aus `mounts.extra_ro` und `mounts.extra_rw`.
    User,
    /// Aus dem Agent-Adapter.
    Adapter,
}

/// Eine Zeile der Tabelle: ein Pfad in der Sandbox und was mit ihm gilt.
///
/// Ein Backend erzeugt die Zeilen beim Übersetzen des Profils aus denselben
/// Daten wie seine Zeile, nicht aus ihr (HUM-429). Unter Seatbelt, ohne
/// eigenen Mount-Namensraum, sind Ziel und Quelle derselbe echte Pfad, und
/// die Namen aus der Profildatei stehen daneben.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRow {
    /// Was mit dem Pfad gilt.
    pub mode: PolicyMode,
    /// Die Quelle auf dem Host, wenn es eine gibt; `None` für Masken,
    /// tmpfs, Verweise, `/proc` und `/dev`.
    pub source: Option<PathBuf>,
    /// Der Pfad, wie der Agent ihn sieht.
    pub target: PathBuf,
    /// Das Ziel eines Verweises, ein Pfad in der Sandbox.
    pub link: Option<PathBuf>,
    /// Woher die Zeile stammt.
    pub origin: PolicyOrigin,
    /// Die Namen, unter denen die Profildatei diesen Pfad führt (`/work`),
    /// wo sie nicht das Ziel sind (Seatbelt).
    pub names: Vec<String>,
}

impl PolicyRow {
    /// Eine Zeile ohne Verweis und ohne weitere Namen.
    #[must_use]
    pub const fn new(
        mode: PolicyMode,
        source: Option<PathBuf>,
        target: PathBuf,
        origin: PolicyOrigin,
    ) -> Self {
        Self {
            mode,
            source,
            target,
            link: None,
            origin,
            names: Vec::new(),
        }
    }
}

/// Die eine Tür in die Sandbox, je Backend (HUM-429).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Door {
    /// Der Proxy-Socket als eine Datei in der Sandbox (bwrap).
    SocketFile {
        /// Der Socket auf dem Host.
        host: PathBuf,
        /// Sein Pfad in der Sandbox.
        sandbox: PathBuf,
    },
    /// Ein Port auf `127.0.0.1`, den der Daemon an den Socket weiterreicht
    /// (Seatbelt).
    LoopbackPort {
        /// Der Socket auf dem Host, an den der Port führt.
        host: PathBuf,
        /// Der Port; `0` in der Vorschau, dort vergibt ihn erst der Start.
        port: u16,
    },
    /// Ein vsock-Port des Gastes, auf dem Host ein Unix-Socket (Kernel-Stufe,
    /// HUM-444).
    VsockPort {
        /// Der Socket auf dem Host.
        host: PathBuf,
        /// Der Port im Gast.
        port: u32,
    },
}

impl Door {
    /// Die Art der Tür in `snake_case`, wie das Protokoll sie führt.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::SocketFile { .. } => "socket_file",
            Self::LoopbackPort { .. } => "loopback_port",
            Self::VsockPort { .. } => "vsock_port",
        }
    }

    /// Die Tür als Text für `Status.door`: die Art und die Adresse, unter der
    /// die Sandbox sie erreicht. Der Port, den erst der Start vergibt, steht
    /// als [`crate::PREVIEW_SESSION_PORT`] da.
    #[must_use]
    pub fn describe(&self) -> String {
        let at = match self {
            Self::SocketFile { sandbox, .. } => sandbox.display().to_string(),
            Self::LoopbackPort { port: 0, .. } => {
                format!("127.0.0.1:{}", crate::PREVIEW_SESSION_PORT)
            }
            Self::LoopbackPort { port, .. } => format!("127.0.0.1:{port}"),
            Self::VsockPort { port, .. } => format!("vsock:{port}"),
        };
        format!("{} {at}", self.kind())
    }
}

/// Grenzen der Mittel einer Sandbox (geplant: Podman, HUM-433). Leer heißt:
/// Das Backend setzt keine; so bei bwrap und Seatbelt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Höchster Speicher in Bytes.
    pub memory_bytes: Option<u64>,
    /// Rechenzeit in Tausendsteln einer CPU.
    pub cpu_millis: Option<u32>,
    /// Höchstzahl der Prozesse.
    pub pids: Option<u32>,
}

impl ResourceLimits {
    /// Ob keine Grenze gesetzt ist.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.memory_bytes.is_none() && self.cpu_millis.is_none() && self.pids.is_none()
    }
}

/// Was eine Sitzung täte, ohne Seiteneffekt; was CLI und Oberfläche zeigen
/// (HUM-429).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanPreview {
    /// Die Kommandozeile, `argv[0]` ist das Programm.
    pub argv: Vec<OsString>,
    /// Die Tabelle der Pfade, in der Reihenfolge des Backends.
    pub policy: Vec<PolicyRow>,
    /// Die Umgebung des Befehls in der Sandbox, alphabetisch.
    pub env: Vec<(String, String)>,
    /// Die Schlüssel aus [`PlanPreview::env`], die allein das Backend setzt
    /// und die keine Quelle des Profils kennt (Seatbelt: `TMPDIR`).
    pub backend_env: Vec<String>,
    /// Wo in [`PlanPreview::argv`] ein Wert aus [`PlanPreview::env`] steht,
    /// als `(Schlüssel, Index)`; leer, wenn das Backend die Umgebung nicht in
    /// der Zeile trägt. Damit hält [`PlanPreview::withhold`] Werte zurück,
    /// ohne dass der Aufrufer die Flags des Backends liest.
    pub argv_env: Vec<(String, usize)>,
    /// Das Regelwerk, das die Sandbox begrenzt, wenn das Backend eines hat:
    /// unter Seatbelt der Text von `sandbox.sb`.
    pub policy_text: Option<String>,
    /// Die eine Tür in die Sandbox.
    pub door: Door,
    /// Die Grenzen der Mittel; bei bwrap und Seatbelt leer.
    pub limits: ResourceLimits,
}

impl PlanPreview {
    /// Ersetzt in [`PlanPreview::argv`] jeden Wert einer Variablen, deren
    /// Wert `shows` nicht zeigen darf, durch `placeholder`.
    ///
    /// Die Entscheidung trifft der Aufrufer an einer Stelle, für Zeile und
    /// Tabelle zugleich; hier steht nur, wo die Werte liegen.
    pub fn withhold(&mut self, shows: impl Fn(&str) -> bool, placeholder: &str) {
        withhold_env(&mut self.argv, &self.argv_env, shows, placeholder);
    }
}

/// Ersetzt in `argv` den Wert an jeder Stelle aus `argv_env`, deren
/// Schlüssel `shows` nicht zeigen darf, durch `placeholder`.
///
/// Gemeinsam für [`PlanPreview::withhold`] und
/// [`LaunchPlan::withheld_argv_line`]: Wo die Werte liegen, weiß das Backend;
/// was gezeigt wird, entscheidet der Aufrufer.
fn withhold_env(
    argv: &mut [OsString],
    argv_env: &[(String, usize)],
    shows: impl Fn(&str) -> bool,
    placeholder: &str,
) {
    for (key, index) in argv_env {
        if !shows(key)
            && let Some(value) = argv.get_mut(*index)
        {
            *value = OsString::from(placeholder);
        }
    }
}

/// Wie die Umgebung des Befehls in die Sandbox kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EnvDelivery {
    /// Sie steht in `argv` (bwrap mit `--setenv`); das Programm selbst
    /// startet mit leerer Umgebung.
    Argv,
    /// Sie ist die Umgebung des Programms, das sie an den Befehl weiterreicht
    /// (`sandbox-exec`, HUM-323).
    Program,
    /// Sie liegt als JSON-Objekt in einem versiegelten memfd, den der Shim
    /// unter dieser Nummer erbt (`--env-fd`, HUM-430,
    /// [`crate::shim_fds::env_memfd`]); das Programm startet mit leerer
    /// Umgebung, und kein Wert steht in `argv`. Für ein Backend, das jeden
    /// Wert in `argv` auf die Platte schriebe (eine Container-Engine, HUM-292 P11). Der
    /// Deskriptor steht unter den geerbten des Plans; die Vorschau zeigt die
    /// Umgebung weiter in ihrer Tabelle ([`PlanPreview::env`]).
    InheritedFd(RawFd),
}

impl EnvDelivery {
    /// Die Umgebung, mit der das gestartete Programm selbst läuft: bei
    /// [`EnvDelivery::Program`] die des Plans, sonst keine. Unter
    /// [`EnvDelivery::InheritedFd`] steht damit kein Wert der Umgebung in
    /// `argv` und keiner in der Umgebung des Programms; er liegt allein im
    /// memfd (HUM-430).
    pub(crate) fn program_env(self, env: &[(String, String)]) -> Vec<(String, String)> {
        match self {
            Self::Program => env.to_vec(),
            Self::Argv | Self::InheritedFd(_) => Vec::new(),
        }
    }

    /// Die Nummer des memfd mit der Umgebung, unter der der Shim ihn erbt,
    /// wenn die Umgebung über einen Deskriptor kommt.
    pub(crate) const fn inherited_fd(self) -> Option<RawFd> {
        match self {
            Self::InheritedFd(fd) => Some(fd),
            Self::Argv | Self::Program => None,
        }
    }
}

/// Wie eine gestartete Sandbox beendet wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Teardown {
    /// `SIGTERM`, nach der Frist `SIGKILL` an den gestarteten Prozess. Mit
    /// ihm endet die ganze Sandbox: bwrap nimmt mit `--die-with-parent` den
    /// PID-Namensraum mit, und unter `sandbox-exec` ist der Prozess selbst
    /// die Sandbox.
    Process,
    /// Die Sandbox ist ein Container, der vom gestarteten Prozess, dem Client
    /// der Engine, abgekoppelt läuft (HUM-292 P7): Ein Signal an den Client
    /// erreicht ihn nicht. Beendet wird er über die Engine selbst, beim
    /// Namen ([`ContainerStop`]), und der Client endet mit ihm.
    Container(ContainerStop),
    /// Die Sandbox ist eine vorübergehende Unit des Dienstverwalters des
    /// Nutzers mit mehreren Prozessen (Kernel-Stufe, HUM-444). Beendet wird
    /// sie über den Dienstverwalter, beim Namen ([`UnitStop`]); mit ihr endet
    /// `systemd-run`, der gestartete Prozess.
    #[cfg_attr(
        not(all(target_os = "linux", target_arch = "x86_64")),
        allow(dead_code, reason = "only the kernel level, Linux on x86-64, uses it")
    )]
    Unit(UnitStop),
}

/// Die Zulassung des Agenten durch den Host (HUM-444): Ein Backend, dessen
/// Shim erst nach der Prüfung des Daemons startet, hinterlegt hier die
/// Verbindung, auf der das Urteil geht. [`Admission::decide`] schickt es
/// genau einmal und merkt es sich: Eine spätere Prüfung derselben Sitzung
/// schickt nichts mehr, ihr Ergebnis gilt aber weiter. Ohne Urteil startet
/// der Agent nie.
#[derive(Debug, Clone, Default)]
pub(crate) struct Admission(std::sync::Arc<Mutex<Verdict>>);

/// Wo das Urteil einer Sitzung steht.
#[derive(Debug, Default)]
enum Verdict {
    /// Noch keins, und noch keine Verbindung dafür.
    #[default]
    Open,
    /// Die Verbindung, auf der es gehen wird.
    Offered(std::os::unix::net::UnixStream),
    /// `ADMIT` ist angekommen; der Agent läuft.
    Admitted,
    /// `DENY` ging, oder kein Urteil erreichte den Gast.
    Refused,
}

impl PartialEq for Admission {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Admission {}

#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    allow(dead_code, reason = "only the kernel level, Linux on x86-64, uses it")
)]
impl Admission {
    /// Legt die Verbindung ab, auf der das Urteil geht.
    pub(crate) fn offer(&self, stream: std::os::unix::net::UnixStream) {
        let mut verdict = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if matches!(*verdict, Verdict::Open) {
            *verdict = Verdict::Offered(stream);
        }
    }

    /// Das Urteil dieser Prüfung. `true` heißt: Der Agent ist zugelassen,
    /// jetzt (`ADMIT` ging hinaus) oder schon bei einer früheren Prüfung, und
    /// diese Prüfung bestand. Beim ersten Urteil geht `ADMIT` oder `DENY` in
    /// den Gast; ohne Verbindung, oder wenn das Schreiben scheitert, ist es
    /// `false`, und der Agent startet nicht. Ein späteres `false` beendet
    /// die Sitzung beim Aufrufer, schickt aber nichts mehr.
    pub(crate) fn decide(&self, admit: bool) -> bool {
        use std::io::Write as _;
        let mut verdict = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match std::mem::take(&mut *verdict) {
            Verdict::Admitted => {
                *verdict = Verdict::Admitted;
                admit
            }
            Verdict::Refused | Verdict::Open => {
                *verdict = Verdict::Refused;
                false
            }
            Verdict::Offered(mut stream) => {
                let line: &[u8] = if admit { b"ADMIT\n" } else { b"DENY\n" };
                let sent = stream.write_all(line).is_ok();
                if admit && sent {
                    *verdict = Verdict::Admitted;
                    true
                } else {
                    *verdict = Verdict::Refused;
                    false
                }
            }
        }
    }
}

/// Wie der Dienstverwalter eine Unit einer Sitzung beendet (HUM-444).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnitStop {
    /// `systemd-run` und `systemctl` mit ihrer Umgebung.
    pub(crate) units: crate::user_unit::UserUnits,
    /// Die Unit der Sitzung.
    pub(crate) unit: crate::user_unit::UnitName,
    /// Das Laufzeitverzeichnis der Sitzung; die Host-Prüfung liest es.
    pub(crate) runtime: PathBuf,
    /// Wohin das Urteil über den Agenten geht.
    pub(crate) admission: Admission,
}

impl UnitStop {
    /// `SIGTERM` stößt den Stopp der Unit an; `SIGKILL` schickt jedem ihrer
    /// Prozesse `SIGKILL` und wartet auf den Stopp. Eine Unit, die es nicht
    /// mehr gibt, nimmt der Dienstverwalter stumm hin.
    pub(crate) fn send(&self, signal: rustix::process::Signal) {
        let wait = std::time::Duration::from_secs(10);
        if signal == rustix::process::Signal::KILL {
            let _ = crate::user_unit::run_bounded(self.units.kill_command(&self.unit), wait);
            let _ = crate::user_unit::run_bounded(self.units.stop_command(&self.unit, true), wait);
        } else {
            let _ = crate::user_unit::run_bounded(self.units.stop_command(&self.unit, false), wait);
        }
    }
}

/// Wie die Engine einen Container beim Namen beendet (HUM-433).
///
/// Die Befehle selbst baut der Adapter der Engine; hier steht nur, was er
/// dafür braucht: das Programm, den Namen des Containers und die Umgebung,
/// mit der das Programm gestartet wird. Die Umgebung trägt nur Pfade und
/// keinen Wert des Agenten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContainerStop {
    /// Das Programm der Engine, aufgelöst.
    pub(crate) program: PathBuf,
    /// Der Name des Containers.
    pub(crate) name: String,
    /// Die ganze Umgebung des Programms; es startet mit keiner anderen.
    pub(crate) env: Vec<(String, String)>,
    /// Wie das Programm startet: als Kind, oder als vorübergehende Unit des
    /// Dienstverwalters des Nutzers (HUM-434).
    pub(crate) start: crate::user_unit::ClientStart,
    /// Die Unit, in der der Client der Sitzung läuft, wenn er über den
    /// Dienstverwalter startete; das Beenden stößt auch ihren Stopp an.
    pub(crate) unit: Option<crate::user_unit::UnitName>,
}

/// Die drei Garantien (BACKLOG.md 4.1, `docs/SECURITY.md` Abschnitt 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IsolationCheck {
    /// Außer `lo` gibt es kein Netzwerk-Interface.
    NoNetworkInterface,
    /// Genau ein Unix-Socket führt hinein, der Proxy.
    SingleSocket,
    /// Der seccomp-Filter ist geladen und antwortet.
    SeccompActive,
}

impl IsolationCheck {
    /// Die drei, in der Reihenfolge des Berichts.
    pub const ALL: [Self; 3] = [
        Self::NoNetworkInterface,
        Self::SingleSocket,
        Self::SeccompActive,
    ];

    /// Der Name in `snake_case`, wie ihn Protokoll und Oberfläche führen.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoNetworkInterface => "no_network_interface",
            Self::SingleSocket => "single_socket",
            Self::SeccompActive => "seccomp_active",
        }
    }
}

/// Das Ergebnis einer Garantie-Prüfung.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckResult {
    /// Welche Garantie.
    pub check: IsolationCheck,
    /// Ob sie gilt.
    pub passed: bool,
    /// Was gesehen wurde, für die Anzeige.
    pub evidence: String,
    /// Der Befund, wenn sie nicht gilt oder nicht geprüft werden konnte.
    pub diagnostic: Option<Diagnostic>,
}

/// Wohin Ein- und Ausgabe der Sandbox gehen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StdioMode {
    /// Die drei Deskriptoren des Aufrufers, unverändert: die Kommandozeile,
    /// ein Terminal. Die Meldungen von `bwrap` landen dann dort, nicht im
    /// Befund.
    #[default]
    Inherit,
    /// Eingabe aus `/dev/null`, Ausgabe und Fehlerausgabe gesammelt
    /// ([`SandboxHandle::output`]). Für Prüfläufe, Tests und kurze Befehle.
    Capture,
    /// Alle drei Deskriptoren an einem Pseudoterminal, dessen Herrscherseite
    /// der Daemon behält (HUM-042).
    ///
    /// Das ist der Modus einer Sitzung, in der ein Mensch mit dem Agenten
    /// arbeitet: Er gibt dem Agenten ein Terminal — `isatty` ist wahr,
    /// `TERM` gilt, ein Vollbild-TUI läuft —, und der Daemon liest und
    /// schreibt die andere Seite ([`SandboxHandle::pty_master`],
    /// [`SandboxHandle::write_input`], [`SandboxHandle::resize`]).
    ///
    /// Was das gegenüber [`StdioMode::Capture`] ändert, und was daran hängt:
    ///
    /// - **Ein Strom statt zwei.** Ein Terminal hat keine getrennte
    ///   Fehlerausgabe; alles kommt als [`crate::OutputStream::Stdout`].
    /// - **Kein `EOF` auf der Eingabe.** Ein Agent, der von stdin liest,
    ///   wartet, statt sofort zu enden.
    /// - **`\n` wird zu `\r\n`.** Das ist die Zeilendisziplin des Terminals
    ///   und nicht der Agent.
    /// - **Die Startdiagnostik kommt aus demselben Strom.** Ohne getrennte
    ///   Fehlerausgabe fände `SandboxHandle::wait` die Meldung von `bwrap`
    ///   nicht; die ersten [`crate::handle::PTY_MIRROR_BYTES`] werden deshalb
    ///   gespiegelt.
    Pty {
        /// Spalten beim Start; der Client ändert sie später mit
        /// [`SandboxHandle::resize`].
        cols: u16,
        /// Zeilen beim Start.
        rows: u16,
    },
}

impl StdioMode {
    /// Die Geometrie, wenn dies ein Pseudoterminal ist.
    #[must_use]
    pub const fn pty_size(self) -> Option<(u16, u16)> {
        match self {
            Self::Pty { cols, rows } => Some((cols, rows)),
            Self::Inherit | Self::Capture => None,
        }
    }
}

/// Was beim Start einmal aus dem Plan genommen wird.
#[derive(Debug)]
pub(crate) struct LaunchOnce {
    /// Die Deskriptoren, die das Programm erbt, unter bwrap die memfds der
    /// Masken und die Schreibseiten der Pipes. Werden nach dem Start geschlossen.
    pub(crate) inherit: Vec<OwnedFd>,
    /// Die Leseseite der Berichts-Pipe des Shims.
    pub(crate) report: PipeReader,
    /// Die Leseseite der Status-Pipe des Programms, wenn es eine hat
    /// (`--json-status-fd` unter bwrap).
    pub(crate) status: Option<PipeReader>,
    /// Wer die Ausgabe mitliest ([`LaunchOptions::output_sink`]); geht mit
    /// dem einen Start mit.
    pub(crate) output_sink: Option<OutputSink>,
    /// Die Seite des Daemons an der Tür zu weiteren Shells (HUM-299), nur
    /// für einen Start am eigenen Pseudoterminal.
    pub(crate) shells: Option<crate::shell::DoorSeed>,
    /// Was ein Start braucht, dessen Sandbox vom Daemon abgekoppelt läuft
    /// (eine Container-Engine): Lebensleine und Übergabe. `None` unter bwrap
    /// und Seatbelt.
    pub(crate) detached: Option<Detached>,
}

/// Lebensleine und Übergabe eines Starts, dessen Sandbox vom Daemon
/// abgekoppelt läuft.
#[derive(Debug)]
pub(crate) struct Detached {
    /// Das Schreibende der Lebensleine (HUM-430), deren Leseende der Shim
    /// bekommt. Es geht mit dem Start in den Zustand der Sandbox und lebt,
    /// bis sie endet oder der Daemon stirbt.
    pub(crate) lifeline: crate::shim_fds::Lifeline,
    /// Der Socket, über den der Shim [`LaunchOnce::inherit`] (und am eigenen
    /// Pseudoterminal die Untertanenseite) bekommt, statt sie zu erben
    /// (HUM-434, [`crate::fd_handover`]). Mit ihm erbt das Programm keinen
    /// Deskriptor; ohne ihn gilt die Vererbung.
    pub(crate) handover: Option<crate::fd_handover::HandoverListener>,
    /// Was ein Backend zwischen Plan und Start für sich selbst trägt (die
    /// Kernel-Stufe: ihre Lauscher und Fäden, HUM-444); `None` sonst.
    #[cfg_attr(
        not(all(target_os = "linux", target_arch = "x86_64")),
        allow(dead_code, reason = "only the kernel level, Linux on x86-64, uses it")
    )]
    pub(crate) backend_state: Option<Box<dyn std::any::Any + Send>>,
}

impl Detached {
    /// Lebensleine und Übergabe, ohne eigenen Zustand des Backends.
    pub(crate) fn handing_over(
        lifeline: crate::shim_fds::Lifeline,
        handover: Option<crate::fd_handover::HandoverListener>,
    ) -> Self {
        Self {
            lifeline,
            handover,
            backend_state: None,
        }
    }
}

/// Die vollständige Kommandozeile samt allem, was der Start braucht.
#[derive(Debug)]
pub struct LaunchPlan {
    /// Das Verzeichnis der Sitzung, wenn das Backend eines anlegt (HUM-368).
    pub session_dir: Option<std::path::PathBuf>,
    /// Die ganze Liste, `argv[0]` ist das Programm. Wird wörtlich angezeigt.
    pub argv: Vec<OsString>,
    /// Wo in [`LaunchPlan::argv`] ein Wert aus [`LaunchPlan::env`] steht, als
    /// `(Schlüssel, Index)`; leer, wenn das Backend die Umgebung nicht in der
    /// Zeile trägt. Wie [`PlanPreview::argv_env`], damit die angezeigte Zeile
    /// eines echten Plans dieselben Werte zurückhalten kann
    /// ([`LaunchPlan::withheld_argv_line`], HUM-450).
    pub argv_env: Vec<(String, usize)>,
    /// Die Umgebung des Befehls in der Sandbox, alphabetisch: dieselben Paare,
    /// die `--setenv` in `argv` trägt. Das Programm selbst startet mit leerer
    /// Umgebung.
    pub env: Vec<(String, String)>,
    /// Die Deskriptoren, die das Programm erbt, als `(host, ziel)`. Beide
    /// Nummern sind gleich: die Deskriptoren werden unter ihrer eigenen Nummer
    /// vererbt, nicht umnummeriert ([`crate::LaunchInputs`]); die Paarform
    /// bleibt, damit ein Backend, das umnummerieren muss, Platz hat.
    pub fds: Vec<(RawFd, RawFd)>,
    /// Die Dateien, die der Agent-Adapter in die Sandbox legt, in der
    /// Reihenfolge, in der sie in [`LaunchPlan::argv`] als `--ro-bind-data`
    /// stehen. Für Anzeige und Prüfung; der Inhalt liegt schon in den
    /// Deskriptoren aus [`LaunchPlan::fds`].
    pub files: Vec<crate::agent::SandboxFile>,
    /// Die Sitzung, für die der Plan gilt.
    pub session: SessionId,
    /// Der Name des Profils, für Protokoll und Thread-Namen.
    pub profile: String,
    /// Befunde, die den Start nicht verhindern, aber dazugehören: eine
    /// aufgehobene Maske (`SANDBOX_020`), ein Kernel ohne `openat2`
    /// (`SANDBOX_021`). Wer den Plan startet, gibt sie in den Ereignisstrom.
    pub warnings: Vec<Diagnostic>,
    /// Die Überdeckungen unter `/work`, die in diesem Lauf **fehlen**, weil es
    /// den Pfad auf dem Host nicht gibt (HUM-043).
    ///
    /// Beide Arten: die Verzeichnisse aus `mounts.tmpfs` und die Dateien aus
    /// `mounts.masked_files`. Die Pfade sind relativ zum Projektverzeichnis,
    /// also in demselben Raum wie die Einträge des Schnappschusses; nur so
    /// lässt sich eine Änderung einer fehlenden Überdeckung zuordnen
    /// (`crate::summary::SessionSummary::set_unprotected`).
    ///
    /// Was der Agent dort schreibt, landet im Projekt — als neue Datei, die im
    /// Diff des Laufs auftaucht. Die Zusammenfassung nennt die Liste, damit die
    /// Lücke benannt ist und nicht bloß vorhanden.
    pub unprotected: Vec<PathBuf>,
    /// Wie [`LaunchPlan::env`] in die Sandbox kommt.
    pub(crate) env_delivery: EnvDelivery,
    /// Die Umgebung des Programms selbst, über das hinaus, was
    /// [`EnvDelivery::program_env`] ihm gibt: unter einer Container-Engine
    /// die Pfade ihrer Konfiguration, `HOME`, `XDG_RUNTIME_DIR` und `PATH`
    /// (HUM-433), sonst leer. Kein Wert des Agenten steht darin.
    pub(crate) program_env: Vec<(String, String)>,
    /// Wohin Ein- und Ausgabe gehen ([`LaunchOptions::stdio`]).
    pub(crate) stdio: StdioMode,
    /// Die Frist für den Bericht des Shims
    /// ([`LaunchOptions::effective_report_timeout`]).
    pub(crate) report_timeout: Duration,
    /// Wie die gestartete Sandbox beendet wird.
    pub(crate) teardown: Teardown,
    /// Ob das Programm die geerbten Deskriptoren fortlaufend ab 3 bekommt,
    /// in der Reihenfolge von [`LaunchOnce::inherit`], statt unter ihrer
    /// eigenen Nummer (HUM-433): Eine Engine reicht nur einen Block ab 3 an
    /// den Container weiter (`--preserve-fds`). Die Ziele stehen in
    /// [`LaunchPlan::fds`]. Am eigenen Pseudoterminal
    /// ([`StdioMode::Pty`]) folgt dann als letzter Deskriptor eine Kopie der
    /// Untertanenseite, die der Shim auf 0, 1 und 2 legt.
    pub(crate) renumbered: bool,
    /// Das Programm, `argv[0]`, aufgelöst.
    pub(crate) program: PathBuf,
    /// Was der Start einmal entnimmt; danach `None`.
    pub(crate) once: Mutex<Option<LaunchOnce>>,
}

impl LaunchPlan {
    /// Das Programm, das startet: `argv[0]` als Pfad.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Die ganze Liste als eine Zeile, nach POSIX zitiert, mit Programm.
    ///
    /// `shlex::split` der Zeile ergibt wieder [`LaunchPlan::argv`]. Ausführbar
    /// ist die Zeile nur mit den Deskriptoren aus [`LaunchPlan::fds`], die ein
    /// Terminal nicht hat; sie ist Anzeige, nicht Ausführung.
    #[must_use]
    pub fn argv_line(&self) -> String {
        shell_line(&self.argv)
    }

    /// Wie [`LaunchPlan::argv_line`], aber mit `placeholder` anstelle jedes
    /// Werts einer Variablen, deren Wert `shows` nicht zeigen darf (HUM-450).
    ///
    /// Für die Anzeige eines Plans, der wirklich startet (`humanitl sandbox
    /// run -v`); gestartet wird weiter [`LaunchPlan::argv`] unverändert.
    #[must_use]
    pub fn withheld_argv_line(&self, shows: impl Fn(&str) -> bool, placeholder: &str) -> String {
        let mut argv = self.argv.clone();
        withhold_env(&mut argv, &self.argv_env, shows, placeholder);
        shell_line(&argv)
    }

    /// Ob der Plan noch nicht gestartet wurde.
    #[must_use]
    pub fn is_fresh(&self) -> bool {
        self.once
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Nimmt, was das Backend zwischen Plan und Start für sich trägt
    /// ([`Detached::backend_state`]); der Rest bleibt für den Start.
    #[cfg_attr(
        not(all(target_os = "linux", target_arch = "x86_64")),
        allow(dead_code, reason = "only the kernel level, Linux on x86-64, uses it")
    )]
    pub(crate) fn take_backend_state(&self) -> Option<Box<dyn std::any::Any + Send>> {
        self.once
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
            .and_then(|once| once.detached.as_mut())
            .and_then(|detached| detached.backend_state.take())
    }

    /// Nimmt, was der Start braucht; beim zweiten Mal `None`.
    pub(crate) fn take_once(&self) -> Option<LaunchOnce> {
        self.once
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// HUM-444: Das Urteil geht einmal; eine spätere bestandene Prüfung
    /// bleibt zugelassen ohne zweites `ADMIT`, eine spätere gescheiterte
    /// nicht. Ohne Verbindung gibt es keine Zulassung.
    #[test]
    #[allow(clippy::unwrap_used)]
    fn the_admission_is_sent_once_and_remembered() {
        use std::io::Read as _;
        let (host, mut guest) = std::os::unix::net::UnixStream::pair().unwrap();
        let admission = Admission::default();
        admission.offer(host);
        assert!(admission.decide(true));
        assert!(
            admission.decide(true),
            "a second healthy check stays admitted"
        );
        assert!(
            !admission.decide(false),
            "a later failed check is not admitted"
        );
        drop(admission);
        let mut seen = String::new();
        guest.read_to_string(&mut seen).unwrap();
        assert_eq!(seen, "ADMIT\n", "exactly one verdict reached the guest");

        let (host, mut guest) = std::os::unix::net::UnixStream::pair().unwrap();
        let refused = Admission::default();
        refused.offer(host);
        assert!(!refused.decide(false));
        assert!(
            !refused.decide(true),
            "a refused agent is never admitted later"
        );
        drop(refused);
        let mut seen = String::new();
        guest.read_to_string(&mut seen).unwrap();
        assert_eq!(seen, "DENY\n");

        assert!(
            !Admission::default().decide(true),
            "no connection, no admission"
        );
    }

    /// Acceptance criterion 2 of HUM-430 on the launcher's side: with an
    /// inherited descriptor the program starts with no environment of its
    /// own, so no value of the plan's environment reaches it except through
    /// the memfd; `sandbox-exec` keeps passing its own.
    #[test]
    fn only_the_program_delivery_hands_the_environment_to_the_program() {
        let env = vec![("HL_SECRET".to_owned(), "s3cr3t".to_owned())];
        assert_eq!(EnvDelivery::Program.program_env(&env), env);
        assert!(EnvDelivery::Argv.program_env(&env).is_empty());
        assert!(EnvDelivery::InheritedFd(7).program_env(&env).is_empty());
        assert_eq!(EnvDelivery::InheritedFd(7).inherited_fd(), Some(7));
        assert_eq!(EnvDelivery::Argv.inherited_fd(), None);
    }

    /// HUM-450: Die angezeigte Zeile eines Plans hält genau die Werte zurück,
    /// die der Aufrufer verbirgt; der Plan selbst bleibt, wie er startet.
    #[test]
    fn the_shown_plan_line_withholds_exactly_the_hidden_values() {
        let plan = LaunchPlan {
            session_dir: None,
            argv: [
                "bwrap", "--setenv", "A", "a1", "--setenv", "B", "b:s3cret", "--", "sh",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
            argv_env: vec![("A".into(), 3), ("B".into(), 6)],
            env: vec![("A".into(), "a1".into()), ("B".into(), "b:s3cret".into())],
            fds: Vec::new(),
            files: Vec::new(),
            session: SessionId::new(),
            profile: "test".into(),
            warnings: Vec::new(),
            unprotected: Vec::new(),
            env_delivery: EnvDelivery::Argv,
            program_env: Vec::new(),
            stdio: StdioMode::Capture,
            report_timeout: Duration::from_secs(1),
            teardown: Teardown::Process,
            renumbered: false,
            program: PathBuf::from("bwrap"),
            once: Mutex::new(None),
        };
        assert_eq!(
            plan.withheld_argv_line(|key| key == "A", "<withheld>"),
            "bwrap --setenv A a1 --setenv B '<withheld>' -- sh"
        );
        assert!(plan.argv_line().contains("b:s3cret"));
    }

    #[test]
    fn isolation_levels_are_ordered_from_policy_to_kernel() {
        assert!(IsolationLevel::Policy < IsolationLevel::Namespaces);
        assert!(IsolationLevel::Namespaces < IsolationLevel::Kernel);
        let mut sorted = [
            IsolationLevel::Kernel,
            IsolationLevel::Policy,
            IsolationLevel::Namespaces,
        ];
        sorted.sort();
        assert_eq!(sorted, IsolationLevel::ALL);
    }

    /// Die Namen werden Werte in `config.toml` und im Protokoll; ein
    /// umbenannter Wert bräche jede gespeicherte Untergrenze.
    #[test]
    fn isolation_level_names_are_stable() {
        let names: Vec<&str> = IsolationLevel::ALL
            .iter()
            .map(|level| level.as_str())
            .collect();
        assert_eq!(names, ["policy", "namespaces", "kernel"]);
    }

    #[test]
    fn default_options_wait_the_default_report_timeout() {
        let options = LaunchOptions::default();
        assert_eq!(options.effective_report_timeout(), REPORT_TIMEOUT);
        assert_eq!(options.stdio, StdioMode::Inherit);
        assert!(options.disposable.is_none() && options.session_ro.is_empty());
        let options = LaunchOptions {
            report_timeout: Some(Duration::from_millis(300)),
            ..LaunchOptions::default()
        };
        assert_eq!(
            options.effective_report_timeout(),
            Duration::from_millis(300)
        );
    }
}
