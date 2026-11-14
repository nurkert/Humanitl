//! Das Backend, das Daemon und Kommandozeile halten (HUM-428), und wie es
//! gewählt wird (HUM-431).
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
//!
//! # Die Wahl (HUM-431)
//!
//! [`choose_backend`] entscheidet vor jedem Start und jeder Vorschau, ohne das
//! System zu fragen, in dieser Reihenfolge:
//!
//! 1. `auto` wird das Backend des Systems ([`crate::PLATFORM_BACKEND_NAME`]).
//! 2. Nennt das Sandbox-Profil ein anderes Backend: `SANDBOX_036`. Das Profil
//!    ist eine Anforderung, keine zweite Wahl (ADR-0011).
//! 3. Liegt die Stufe unter `sandbox.min_isolation`: `SANDBOX_035`.
//! 4. Steht die Stufe nicht in [`ADMITTED_BACKENDS`]: `SANDBOX_048`.
//!
//! Erst danach fragt [`Backend::detect`] das System; ein Backend, das dieser
//! Build nicht hat, ist `SANDBOX_034`. Keiner der Schritte fällt auf ein
//! anderes Backend zurück.

use std::borrow::Cow;
use std::path::Path;

use humanitl_config::{Paths, PodmanRuntime, SandboxBackendName, SandboxRef};
use humanitl_core::diagnostics::codes::{
    CONFIG_003, SANDBOX_034, SANDBOX_035, SANDBOX_036, SANDBOX_048,
};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::bwrap::BwrapBackend;
use crate::handle::SandboxHandle;
use crate::launcher::{
    CheckResult, IsolationLevel, LaunchPlan, PlanPreview, PlanRequest, PullCancel, PulledImage,
    SandboxBackend,
};
pub use crate::podman::with_image_root;
use crate::podman::{PodmanBackend, SHARED_KERNEL_STAGE, static_lines as engine_static_lines};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use crate::qemu::{self as vm, VmBackend};

#[cfg(target_os = "macos")]
use crate::seatbelt::SeatbeltBackend;
/// Kernel und Module der Tests für die Kernel-Stufe (HUM-445).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[cfg(any(test, feature = "unadmitted-backends"))]
pub use vm::with_test_guest_kernel;

/// Der Name von bwrap in `sandbox.backend`.
pub const BWRAP_BACKEND: &str = "bwrap";

/// Der Name von Seatbelt in `sandbox.backend`, nur unter macOS ein Backend.
pub const SEATBELT_BACKEND: &str = "seatbelt";

/// Der Name der Container-Engine in `sandbox.backend` (HUM-431). Das Backend gibt es
/// seit HUM-433; zugelassen ist seine gemeinsame Stufe seit HUM-448.
pub const PODMAN_BACKEND: &str = "podman";

/// Der Name des VM-Backends der Kernel-Stufe in `sandbox.backend` (HUM-464,
/// ADR-0021), nur unter Linux auf x86-64. Den Adapter gibt es seit HUM-444;
/// zugelassen wird er mit HUM-449.
pub const VM_BACKEND: &str = "qemu";

/// Die Stufen, die ein Mensch wählen kann (HUM-431).
///
/// Eine Stufe ist ein Backend, unter der Container-Engine mit ihrer Laufzeit.
/// Sie kommt erst auf diese Liste, wenn ihre Einhänge-Prüfung gelandet und die
/// Escape-Suite gegen sie in der CI grün ist (Tor in `backlog/sprint-7.md`):
/// die gemeinsame Stufe mit HUM-448, das VM-Backend der Kernel-Stufe
/// ([`VM_BACKEND`]) mit HUM-449. Bis dahin lehnt jeder Start eine Stufe mit
/// `SANDBOX_048` ab, und `auto` löst nie auf die Container-Engine oder das
/// VM-Backend auf, auch nicht auf eine zugelassene Stufe.
pub const ADMITTED_BACKENDS: &[&str] = &[BWRAP_BACKEND, SHARED_KERNEL_STAGE, SEATBELT_BACKEND];

/// Ruft `$call` auf dem gewählten Backend auf, ein Zweig je Variante.
macro_rules! dispatch {
    ($self:ident, $backend:ident => $call:expr) => {
        match $self {
            Backend::Bwrap($backend) => $call,
            Backend::Podman($backend) => $call,
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            Backend::Vm($backend) => $call,
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
    /// Die Container-Engine, gemeinsame Stufe (HUM-433). Bauen lässt sie sich nur über
    /// [`Backend::detect`] und [`Backend::fallback`], die die Zulassung
    /// prüfen, und über die Umgehung der Escape-Suite.
    Podman(PodmanBackend),
    /// Das VM-Backend der Kernel-Stufe (HUM-444), nur unter Linux auf x86-64.
    /// Bauen lässt es sich nur über die Umgehung der Escape-Suite
    /// (`Backend::detect_unadmitted`), bis HUM-449 es zulässt.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    Vm(VmBackend),
    /// Seatbelt mit `sandbox-exec`, unter macOS (HUM-323).
    #[cfg(target_os = "macos")]
    Seatbelt(SeatbeltBackend),
}

/// Was [`choose_backend`] entschieden hat: das Backend, seine Stufe und die
/// Isolation, die es bietet. Noch nichts davon ist auf dem System geprüft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendChoice {
    /// Der Name des Backends, `auto` schon aufgelöst.
    pub name: &'static str,
    /// Die Laufzeit, falls das Backend eine hat (die Container-Engine).
    pub runtime: Option<PodmanRuntime>,
    /// Die Isolation, die diese Stufe bietet.
    pub isolation: IsolationLevel,
}

impl BackendChoice {
    /// Der Name der Stufe, wie ihn [`ADMITTED_BACKENDS`] führt: das Backend,
    /// unter Podman mit Laufzeit (`podman/crun`).
    #[must_use]
    pub fn stage(&self) -> Cow<'static, str> {
        stage(self.name, self.runtime)
    }
}

/// Wählt das Backend einer Sitzung aus der Konfiguration und dem
/// Sandbox-Profil (HUM-431), ohne das System zu fragen.
///
/// `sandbox` ist die aufgelöste Gruppe `sandbox` (Backend, Untergrenze,
/// Laufzeit unter Podman), `profile_backend` das Feld `[sandbox] backend` des
/// Sandbox-Profils. Die Reihenfolge der Prüfungen steht in der
/// Modulbeschreibung.
///
/// # Errors
///
/// `SANDBOX_036`, wenn das Profil ein anderes Backend verlangt,
/// `SANDBOX_035`, wenn die Stufe unter `sandbox.min_isolation` liegt,
/// `SANDBOX_048`, wenn sie nicht zugelassen ist, `CONFIG_003` für einen Namen
/// im Profil, den dieser Build nicht kennt.
pub fn choose_backend(
    sandbox: &SandboxRef,
    profile_backend: &str,
) -> Result<BackendChoice, Diagnostic> {
    let name = resolve(backend_name(sandbox.backend))?;
    let runtime = (name == PODMAN_BACKEND).then_some(sandbox.podman.runtime);
    let choice = BackendChoice {
        name,
        runtime,
        isolation: isolation_of(name, runtime),
    };
    if let Some(required) = required_by_profile(profile_backend)?
        && required != name
    {
        return Err(profile_requires_another(required, name));
    }
    let floor = IsolationLevel::from(sandbox.min_isolation);
    if choice.isolation < floor {
        return Err(below_floor(&choice, floor));
    }
    admitted(name, runtime)?;
    Ok(choice)
}

/// Lehnt eine Laufzeit ab, die ein Mensch für diesen Lauf gewünscht hat,
/// wenn das Backend gar nicht Podman ist (HUM-432).
///
/// `humanitl run --podman-runtime crun` bei `sandbox.backend = "auto"` hieße
/// sonst: Der Mensch verlangt die Container-Engine und bekommt still bwrap. Als Wunsch gilt eine Laufzeit aus der Umgebung
/// (`HUMANITL_SANDBOX__PODMAN__RUNTIME`) oder von der Kommandozeile, wozu
/// auch `cli_overrides` des Start-RPC gehören; `runtime_origin` ist ihre
/// Herkunft aus der Auflösung. Eine Laufzeit aus `config.toml` oder einem
/// Profil ist eine Vorgabe für später und hält keinen Start auf.
///
/// # Errors
///
/// `SANDBOX_034`: Die gewünschte Laufzeit gibt es mit dem gewählten Backend
/// nicht, und der Start nimmt nicht still eine andere Stufe.
pub fn runtime_fits_backend(
    sandbox: &SandboxRef,
    runtime_origin: Option<&humanitl_config::Origin>,
) -> Result<(), Diagnostic> {
    use humanitl_config::Origin;
    if !matches!(runtime_origin, Some(Origin::Env(_) | Origin::Cli)) {
        return Ok(());
    }
    let name = resolve(backend_name(sandbox.backend))?;
    if name == PODMAN_BACKEND {
        return Ok(());
    }
    Err(Diagnostic::builder(SANDBOX_034, Severity::Blocking)
        .why(format!(
            "sandbox.podman.runtime = {:?} was asked for this run, and sandbox.backend chose \
             {name:?}, which has no such runtime; the start does not run another isolation \
             level than the one asked for. Start with --backend {PODMAN_BACKEND} for that \
             runtime, or without --podman-runtime",
            sandbox.podman.runtime.as_str()
        ))
        .fix(FixAction::CopyCommand(
            "humanitl sandbox backends".to_owned(),
        ))
        .build())
}

impl Backend {
    /// Sucht das Backend mit diesem Namen und prüft, ob es auf dieser
    /// Maschine läuft. [`crate::AUTO_BACKEND`] ist das Backend des Systems
    /// ([`crate::PLATFORM_BACKEND_NAME`]). Ein Backend, das nicht in
    /// [`ADMITTED_BACKENDS`] steht, startet hier nie.
    ///
    /// `sandbox` ist die aufgelöste Gruppe `sandbox`; das Backend liest
    /// daraus, was es betrifft (die Container-Engine ihre eigene Gruppe, HUM-433).
    ///
    /// # Errors
    ///
    /// `SANDBOX_048` für eine Stufe, die nicht zugelassen ist, `SANDBOX_034`
    /// für ein Backend, das dieser Build oder dieses System nicht hat, die
    /// Befunde des Backends (`SANDBOX_001` bis `SANDBOX_003`, bei der Container-Engine
    /// `SANDBOX_037` bis `SANDBOX_040` und `SANDBOX_044`, beim VM-Backend
    /// `SANDBOX_045`), oder `CONFIG_003` für einen Namen, den das Schema nicht
    /// kennt.
    pub fn detect(name: &str, paths: Paths, sandbox: &SandboxRef) -> Result<Self, Diagnostic> {
        let name = resolve(name)?;
        admitted(name, runtime_of(name, sandbox))?;
        detect_in_build(name, paths, sandbox)
    }

    /// Wie [`Backend::detect`], aber ohne die Zulassung: nur für die
    /// Escape-Suite (`escape-launch`) und die Tests, die eine Stufe prüfen
    /// müssen, bevor sie zugelassen ist (HUM-431).
    ///
    /// Steht hinter dem Feature `unadmitted-backends` dieser Crate, das nur
    /// `escape-launch` (über `required-features`) und die Tests einschalten.
    /// `humanitld` und `humanitl` schalten es nie ein; im ausgelieferten Code
    /// gibt es diese Funktion deshalb gar nicht, nicht nur keinen Aufruf.
    ///
    /// # Errors
    ///
    /// Wie [`Backend::detect`], ohne `SANDBOX_048`.
    #[cfg(any(test, feature = "unadmitted-backends"))]
    pub fn detect_unadmitted(
        name: &str,
        paths: Paths,
        sandbox: &SandboxRef,
    ) -> Result<Self, Diagnostic> {
        detect_in_build(resolve(name)?, paths, sandbox)
    }

    /// Das Backend mit diesem Namen ohne Prüfung des Systems: der Rückfall,
    /// wenn [`Backend::detect`] scheitert und trotzdem ein Plan gezeigt
    /// werden soll. Ob das Programm da ist und läuft, sagt dann erst der
    /// Start, mit dem Befund des Backends. Die Zulassung gilt auch hier.
    ///
    /// # Errors
    ///
    /// `SANDBOX_048`, `SANDBOX_034` oder `CONFIG_003` wie [`Backend::detect`].
    pub fn fallback(name: &str, paths: Paths, sandbox: &SandboxRef) -> Result<Self, Diagnostic> {
        let name = resolve(name)?;
        admitted(name, runtime_of(name, sandbox))?;
        match name {
            BWRAP_BACKEND => Ok(Self::Bwrap(BwrapBackend::fallback(paths))),
            PODMAN_BACKEND => Ok(Self::Podman(PodmanBackend::fallback(
                paths,
                &sandbox.podman,
            ))),
            #[cfg(target_os = "macos")]
            SEATBELT_BACKEND => Ok(Self::Seatbelt(SeatbeltBackend::fallback(paths))),
            other => Err(not_in_build(other)),
        }
    }

    /// Das Programm, das startet: `bwrap`, `sandbox-exec` oder die Engine.
    #[must_use]
    pub fn program(&self) -> &Path {
        dispatch!(self, backend => backend.program())
    }

    /// Die Fassung des Programms als Text, leer, wo das Backend keine eigene
    /// hat; der Schlüssel des Zwischenspeichers im Doctor (HUM-436).
    #[must_use]
    pub fn tool_version(&self) -> String {
        dispatch!(self, backend => backend.tool_version())
    }

    /// Die Pfade, gegen die das Backend plant.
    #[must_use]
    pub const fn paths(&self) -> &Paths {
        match self {
            Self::Bwrap(backend) => backend.paths(),
            Self::Podman(backend) => backend.paths(),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            Self::Vm(backend) => backend.paths(),
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

    fn pull_image(&self, reference: &str, cancel: &PullCancel) -> Result<PulledImage, Diagnostic> {
        dispatch!(self, backend => backend.pull_image(reference, cancel))
    }
}

/// Der Name eines Werts von `sandbox.backend`, wie ihn `config.toml`, das
/// Sandbox-Profil und die Leitung schreiben (HUM-431). Er steht hier und nicht
/// in der Konfigurations-Crate, damit der Name eines Adapters außerhalb des
/// Sandbox-Adapters nur im Schema vorkommt (HUM-145).
#[must_use]
pub const fn backend_name(name: SandboxBackendName) -> &'static str {
    match name {
        SandboxBackendName::Auto => crate::AUTO_BACKEND,
        SandboxBackendName::Bubblewrap => BWRAP_BACKEND,
        SandboxBackendName::Podman => PODMAN_BACKEND,
        SandboxBackendName::Seatbelt => SEATBELT_BACKEND,
        SandboxBackendName::Qemu => VM_BACKEND,
    }
}

/// Der Name des Backends, mit [`crate::AUTO_BACKEND`] aufgelöst. Jeder Name
/// des Schemas ist bekannt, auch einer, den dieser Build nicht starten kann;
/// das sagt erst [`detect_in_build`] mit `SANDBOX_034`.
pub(crate) fn resolve(name: &str) -> Result<&'static str, Diagnostic> {
    match name {
        crate::AUTO_BACKEND => Ok(crate::PLATFORM_BACKEND_NAME),
        BWRAP_BACKEND => Ok(BWRAP_BACKEND),
        SEATBELT_BACKEND => Ok(SEATBELT_BACKEND),
        PODMAN_BACKEND => Ok(PODMAN_BACKEND),
        VM_BACKEND => Ok(VM_BACKEND),
        other => Err(unknown(other)),
    }
}

/// Sucht ein aufgelöstes Backend auf diesem System, ohne Zulassung.
pub(crate) fn detect_in_build(
    name: &'static str,
    paths: Paths,
    sandbox: &SandboxRef,
) -> Result<Backend, Diagnostic> {
    match name {
        BWRAP_BACKEND => BwrapBackend::detect(paths).map(Backend::Bwrap),
        PODMAN_BACKEND => PodmanBackend::detect(paths, &sandbox.podman).map(Backend::Podman),
        // Fehlt eine Voraussetzung, startet nichts, und kein anderes Backend
        // springt ein (HUM-444).
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        VM_BACKEND => vm::detect(paths, sandbox).map(Backend::Vm),
        #[cfg(target_os = "macos")]
        SEATBELT_BACKEND => SeatbeltBackend::detect(paths).map(Backend::Seatbelt),
        other => Err(not_in_build(other)),
    }
}

/// Was dem Rechner für die Kernel-Stufe fehlt (HUM-464): die erste fehlende
/// Voraussetzung als `SANDBOX_045`, oder keine. Ohne Linux auf x86-64 gibt es
/// die Stufe nicht (`SANDBOX_034`).
#[cfg_attr(
    not(all(target_os = "linux", target_arch = "x86_64")),
    expect(
        clippy::unnecessary_wraps,
        reason = "Ohne Linux auf x86-64 fehlt immer etwas; die Signatur ist die der Linux-Fassung"
    )
)]
pub(crate) fn kernel_level_missing(paths: &Paths, sandbox: &SandboxRef) -> Option<Diagnostic> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        vm::missing(paths, sandbox)
    }
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    {
        let _ = (paths, sandbox);
        Some(not_in_build(VM_BACKEND))
    }
}

/// Die Laufzeit, die ein Backend ohne Angabe hat: unter Podman `crun`.
fn default_runtime(name: &str) -> Option<PodmanRuntime> {
    (name == PODMAN_BACKEND).then_some(PodmanRuntime::Crun)
}

/// Die Laufzeit, die die Konfiguration einem Backend gibt: der Container-Engine
/// ihre eingestellte, sonst keine.
pub(crate) fn runtime_of(name: &str, sandbox: &SandboxRef) -> Option<PodmanRuntime> {
    (name == PODMAN_BACKEND).then_some(sandbox.podman.runtime)
}

/// Was eine frühere Sitzung dieses Daemons außerhalb seiner Prozesse liegen
/// ließ (HUM-433): unter Linux die Container der Engine mit den Labels
/// dieser Instanz und einer Sitzung.
///
/// Zwei Schritte, damit nichts weggeräumt wird, was nach dem Start des
/// Daemons entstand: [`LeftSandboxes::find`] nimmt die Bestandsaufnahme
/// synchron und begrenzt, bevor der Daemon einen Start annimmt;
/// [`LeftSandboxes::remove`] entfernt danach genau diese, gern nebenher.
#[derive(Debug, Default)]
pub struct LeftSandboxes {
    found: crate::podman::Leftovers,
    /// Sitzungen der Kernel-Stufe: Laufzeitverzeichnisse und ihre Units
    /// (HUM-444).
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    vm: vm::Leftovers,
}

impl LeftSandboxes {
    /// Die Bestandsaufnahme. Ohne Engine auf dem System, und unter macOS,
    /// ist sie leer. Blockiert, solange die Engine antwortet, höchstens
    /// zehn Sekunden.
    #[must_use]
    pub fn find(paths: &Paths, sandbox: &SandboxRef) -> Self {
        if cfg!(target_os = "linux") {
            Self {
                found: crate::podman::find_leftovers(paths, &sandbox.podman),
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                vm: vm::find_leftovers(paths),
            }
        } else {
            Self::default()
        }
    }

    /// Entfernt, was [`LeftSandboxes::find`] fand. Je Fund ein `SANDBOX_041`
    /// als Warnung, entfernt oder nicht entfernbar, und eines, wenn die
    /// Bestandsaufnahme selbst scheiterte.
    #[must_use]
    pub fn remove(self) -> Vec<Diagnostic> {
        #[cfg_attr(
            not(all(target_os = "linux", target_arch = "x86_64")),
            allow(unused_mut)
        )]
        let mut removed = self.found.remove();
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        removed.extend(self.vm.remove());
        removed
    }
}

/// Der Name der Stufe aus Backend und Laufzeit.
pub(crate) fn stage(name: &'static str, runtime: Option<PodmanRuntime>) -> Cow<'static, str> {
    match runtime {
        Some(runtime) => Cow::Owned(format!("{name}/{}", runtime.as_str())),
        None => Cow::Borrowed(name),
    }
}

/// Die Voraussetzungen eines Backends, die sich ohne einen Start seines
/// Programms prüfen lassen, als Zeilen des Doctors (HUM-436); leer für ein
/// Backend, dessen Suche nichts davon kennt. Je Zeile die Kennung und bei
/// Erfolg der Beleg, sonst der Befund.
pub(crate) fn static_checks(
    name: &str,
    paths: &Paths,
    sandbox: &SandboxRef,
) -> Vec<(crate::doctor::BackendCheckId, Result<String, Diagnostic>)> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    if name == VM_BACKEND {
        return vm::static_lines(paths, sandbox);
    }
    engine_static_lines(name, paths, sandbox)
}

/// Die Isolation einer Stufe, ohne sie zu bauen. Muss mit
/// [`SandboxBackend::isolation`] des gebauten Backends übereinstimmen; der
/// Test `the_isolation_of_a_name_is_that_of_its_backend` hält beides gleich.
pub(crate) fn isolation_of(name: &str, runtime: Option<PodmanRuntime>) -> IsolationLevel {
    match (name, runtime) {
        (SEATBELT_BACKEND, _) => IsolationLevel::Policy,
        (VM_BACKEND, _) => IsolationLevel::Kernel,
        _ => IsolationLevel::Namespaces,
    }
}

/// Das Backend, das ein Sandbox-Profil verlangt, oder keins.
///
/// `auto` verlangt keins. `bwrap` ist dort, wo das System ein anderes
/// Backend hat (macOS), der Wert älterer Profile von vor HUM-376 und bleibt
/// gültig, ohne etwas zu verlangen; dort, wo es bwrap gibt, ist es eine
/// Anforderung wie jede andere.
pub(crate) fn required_by_profile(
    profile_backend: &str,
) -> Result<Option<&'static str>, Diagnostic> {
    if profile_backend == crate::AUTO_BACKEND {
        return Ok(None);
    }
    let required = resolve(profile_backend)?;
    if required == BWRAP_BACKEND && crate::PLATFORM_BACKEND_NAME != BWRAP_BACKEND {
        return Ok(None);
    }
    Ok(Some(required))
}

/// `SANDBOX_048`, wenn die Stufe nicht in [`ADMITTED_BACKENDS`] steht.
pub(crate) fn admitted(
    name: &'static str,
    runtime: Option<PodmanRuntime>,
) -> Result<(), Diagnostic> {
    let stage = stage(name, runtime);
    if ADMITTED_BACKENDS.contains(&stage.as_ref()) {
        return Ok(());
    }
    // Die Stufe nur, wo sie mehr sagt als der Name, etwa mit Laufzeit.
    let named = if stage == name {
        format!("{name:?}")
    } else {
        format!("{name:?} ({stage})")
    };
    Err(Diagnostic::builder(SANDBOX_048, Severity::Blocking)
        .why(format!(
            "sandbox.backend = {named} is not admitted yet: a backend becomes selectable only \
             after its mount check and the escape suite against it are green in CI. Admitted \
             are: {}",
            ADMITTED_BACKENDS.join(", ")
        ))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.backend".to_owned(),
            value: crate::AUTO_BACKEND.to_owned(),
        })
        .build())
}

/// `SANDBOX_034`: ein Name des Schemas, den dieser Build oder dieses System
/// nicht starten kann. Nie ein stiller Rückfall auf ein anderes Backend.
fn not_in_build(name: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_034, Severity::Blocking)
        .why(format!(
            "sandbox.backend = {name:?} is not available in this build on this system; the \
             backend of this system is \"{}\", and the start does not switch to it on its own",
            crate::PLATFORM_BACKEND_NAME
        ))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.backend".to_owned(),
            value: crate::AUTO_BACKEND.to_owned(),
        })
        .build()
}

/// `SANDBOX_035`: Die Stufe liegt unter der Untergrenze. Ohne Knopf: Die
/// Untergrenze zu senken entscheidet der Mensch in der Ebene, die sie gesetzt
/// hat, und die Antwort darf ihm diese Entscheidung nicht abnehmen.
pub(crate) fn below_floor(choice: &BackendChoice, floor: IsolationLevel) -> Diagnostic {
    Diagnostic::builder(SANDBOX_035, Severity::Blocking)
        .why(format!(
            "sandbox.backend chose {} with the isolation level {}, and sandbox.min_isolation \
             asks for at least {}; the floor is the strictest value of all layers, and the start \
             does not pick another backend on its own",
            choice.stage(),
            choice.isolation.as_str(),
            floor.as_str()
        ))
        .fix(FixAction::CopyCommand(
            "humanitl config get --origin sandbox.min_isolation".to_owned(),
        ))
        .build()
}

/// `SANDBOX_036`: Das Sandbox-Profil verlangt ein anderes Backend.
pub(crate) fn profile_requires_another(required: &'static str, chosen: &'static str) -> Diagnostic {
    let builder = Diagnostic::builder(SANDBOX_036, Severity::Blocking).why(format!(
        "the sandbox profile requires the backend {required:?}, and sandbox.backend chose \
         {chosen:?}; the profile states a requirement, not a second choice, so neither \
         overrides the other"
    ));
    // Ein Knopf nur zu einem Backend, das zugelassen ist: Sonst führte er
    // geradewegs in `SANDBOX_048`.
    let builder = if admitted(required, default_runtime(required)).is_ok() {
        builder.fix(FixAction::ChangeSetting {
            key: "sandbox.backend".to_owned(),
            value: required.to_owned(),
        })
    } else {
        builder
    };
    builder.build()
}

/// `CONFIG_003`: ein Name, den das Schema nicht kennt. Nie ein stiller
/// Rückfall auf ein anderes Backend.
fn unknown(name: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Blocking)
        .why(format!(
            "sandbox.backend = {name:?}, the known names are \"{}\", \"{BWRAP_BACKEND}\", \
             \"{PODMAN_BACKEND}\", \"{SEATBELT_BACKEND}\" and \"{VM_BACKEND}\"",
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
        let err = Backend::detect(
            BWRAP_BACKEND,
            without_bwrap(dir.path()),
            &SandboxRef::default(),
        )
        .expect_err("no bwrap in PATH");
        assert_eq!(err.code.as_str(), "SANDBOX_001", "{}", err.why);
    }

    #[test]
    fn an_unknown_name_is_refused_and_never_falls_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        for name in ["docker", "", "BWRAP"] {
            let err = Backend::detect(name, without_bwrap(dir.path()), &SandboxRef::default())
                .expect_err("unknown");
            assert_eq!(err.code.as_str(), "CONFIG_003", "{name:?}: {}", err.why);
            let err = Backend::fallback(name, without_bwrap(dir.path()), &SandboxRef::default())
                .expect_err("unknown");
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
        let backend = Backend::fallback(
            BWRAP_BACKEND,
            without_bwrap(dir.path()),
            &SandboxRef::default(),
        )
        .expect("bwrap");
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
        let auto = Backend::fallback(
            crate::AUTO_BACKEND,
            without_bwrap(dir.path()),
            &SandboxRef::default(),
        )
        .expect("auto is known");
        assert_eq!(auto.name(), crate::PLATFORM_BACKEND_NAME);
        let bwrap = Backend::fallback(
            BWRAP_BACKEND,
            without_bwrap(dir.path()),
            &SandboxRef::default(),
        )
        .expect("bwrap");
        assert_eq!(bwrap.name(), BWRAP_BACKEND);
        assert_eq!(bwrap.isolation(), IsolationLevel::Namespaces);
        assert_eq!(bwrap.program(), Path::new("bwrap"));
    }

    /// Eine Laufzeit, die der Lauf wünscht, passt nur zu Podman; aus
    /// `config.toml` hält sie keinen Start auf (HUM-432).
    #[test]
    fn a_runtime_wished_for_another_backend_refuses_the_start() {
        use humanitl_config::MinIsolation::Policy;
        use humanitl_config::Origin;
        use humanitl_config::SandboxBackendName::{Auto, Podman, Qemu};
        for backend in [Auto, Qemu] {
            let other = sandbox(backend, Policy);
            for wish in [
                Origin::Cli,
                Origin::Env("HUMANITL_SANDBOX__PODMAN__RUNTIME".into()),
            ] {
                let err = runtime_fits_backend(&other, Some(&wish)).expect_err("no podman");
                assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
                assert!(err.why.contains("crun"), "{}", err.why);
            }
            for preset in [None, Some(Origin::Default), Some(Origin::Global)] {
                runtime_fits_backend(&other, preset.as_ref()).expect("a preset is no wish");
            }
        }
        let podman = sandbox(Podman, Policy);
        runtime_fits_backend(&podman, Some(&Origin::Cli)).expect("podman has runtimes");
    }

    /// Die Gruppe `sandbox` mit Backend und Untergrenze.
    fn sandbox(
        backend: humanitl_config::SandboxBackendName,
        floor: humanitl_config::MinIsolation,
    ) -> SandboxRef {
        SandboxRef {
            backend,
            min_isolation: floor,
            ..SandboxRef::default()
        }
    }

    fn code_of(result: Result<BackendChoice, Diagnostic>) -> String {
        match result {
            Ok(choice) => panic!("{} was expected to be refused", choice.stage()),
            Err(diagnostic) => diagnostic.code.as_str().to_owned(),
        }
    }

    /// Die Zulassung seit HUM-448: bwrap, Seatbelt und die gemeinsame Stufe
    /// der Container-Engine; das VM-Backend der Kernel-Stufe bleibt bis
    /// HUM-449 draußen, auch unter der Untergrenze `kernel`, die es erfüllt.
    #[test]
    fn the_shared_stage_is_admitted_and_the_kernel_stage_is_not() {
        use humanitl_config::MinIsolation::{Kernel, Policy};
        use humanitl_config::SandboxBackendName::{Podman, Qemu};
        assert_eq!(
            ADMITTED_BACKENDS,
            [BWRAP_BACKEND, "podman/crun", SEATBELT_BACKEND]
        );
        assert_eq!(
            stage(PODMAN_BACKEND, Some(PodmanRuntime::Crun)),
            SHARED_KERNEL_STAGE
        );
        for refused in [PODMAN_BACKEND, VM_BACKEND] {
            assert!(!ADMITTED_BACKENDS.contains(&refused), "{refused}");
        }
        let chosen = choose_backend(&sandbox(Podman, Policy), "auto").expect("admitted");
        assert_eq!(chosen.stage(), SHARED_KERNEL_STAGE);
        assert_eq!(chosen.isolation, IsolationLevel::Namespaces);
        for floor in [Policy, Kernel] {
            let err = choose_backend(&sandbox(Qemu, floor), "auto").expect_err("not admitted");
            assert_eq!(err.code.as_str(), "SANDBOX_048", "{}", err.why);
            assert!(
                err.why.starts_with(&format!(
                    "sandbox.backend = {VM_BACKEND:?} is not admitted yet"
                )),
                "{}",
                err.why
            );
        }
    }

    /// Das VM-Backend der Kernel-Stufe scheitert vor HUM-449 auf jedem Weg in
    /// den Start an der Zulassung. Nur die Umgehung kommt daran vorbei; auf
    /// diesem Rechner ohne `bwrap` endet sie an der Prüfung der
    /// Voraussetzungen, und gestartet wird nie etwas.
    #[test]
    fn the_kernel_level_starts_on_no_path_before_its_admission() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = without_bwrap(dir.path());
        let kernel = SandboxRef {
            backend: humanitl_config::SandboxBackendName::Qemu,
            ..SandboxRef::default()
        };
        for refused in [
            Backend::detect(VM_BACKEND, paths.clone(), &kernel).map(drop),
            Backend::fallback(VM_BACKEND, paths.clone(), &kernel).map(drop),
        ] {
            let err = refused.expect_err("not admitted");
            assert_eq!(err.code.as_str(), "SANDBOX_048", "{}", err.why);
        }
        let err = Backend::detect_unadmitted(VM_BACKEND, paths, &kernel)
            .map(drop)
            .expect_err("a prerequisite is missing");
        assert!(
            ["SANDBOX_045", "SANDBOX_034"].contains(&err.code.as_str()),
            "{}: {}",
            err.code,
            err.why
        );
    }

    /// `sandbox.backend = "podman"` kommt seit HUM-448 auf jedem Weg in den
    /// Start an der Zulassung vorbei bis zum Adapter, der auf diesem `PATH`
    /// keine Engine findet.
    #[test]
    fn the_engine_passes_admission_on_every_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = without_bwrap(dir.path());
        let shared = SandboxRef::default();
        let err = Backend::detect(PODMAN_BACKEND, paths.clone(), &shared).expect_err("no engine");
        assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
        assert!(err.why.contains("no executable podman"), "{}", err.why);
        let preview = Backend::fallback(PODMAN_BACKEND, paths, &shared).expect("admitted");
        assert_eq!(preview.name(), PODMAN_BACKEND);
        assert_eq!(preview.isolation(), IsolationLevel::Namespaces);
    }

    /// `auto` löst auf das Backend des Systems auf und nie auf ein anderes:
    /// Liegt es unter der Untergrenze, lehnt die Wahl ab, statt zu Podman
    /// oder zur Kernel-Stufe zu greifen. Die Untergrenze `kernel` erfüllt nur
    /// das VM-Backend (HUM-464).
    #[test]
    fn a_floor_above_the_backend_refuses_instead_of_choosing_another() {
        use humanitl_config::MinIsolation::{Kernel, Namespaces, Policy};
        use humanitl_config::SandboxBackendName::{Auto, Bubblewrap as Bwrap, Podman, Qemu};
        assert_eq!(
            code_of(choose_backend(&sandbox(Bwrap, Kernel), "auto")),
            "SANDBOX_035"
        );
        assert_eq!(
            code_of(choose_backend(&sandbox(Auto, Kernel), "auto")),
            "SANDBOX_035"
        );
        // Podman teilt den Kern: unter `kernel` ist das 035, noch bevor die
        // Zulassung gefragt wird (HUM-432 baut darauf).
        let err = choose_backend(&sandbox(Podman, Kernel), "auto").expect_err("below");
        assert_eq!(err.code.as_str(), "SANDBOX_035", "{}", err.why);
        assert!(err.why.contains("podman/crun"), "{}", err.why);
        assert!(err.why.contains("kernel"), "{}", err.why);
        // Das VM-Backend liegt nicht darunter; es scheitert nur an der
        // Zulassung.
        assert_eq!(
            code_of(choose_backend(&sandbox(Qemu, Kernel), "auto")),
            "SANDBOX_048"
        );
        // Die Untergrenze selbst und alles darunter geht.
        let platform = choose_backend(&sandbox(Auto, Policy), "auto").expect("default");
        assert_eq!(platform.name, crate::PLATFORM_BACKEND_NAME);
        if cfg!(not(target_os = "macos")) {
            let bwrap = choose_backend(&sandbox(Bwrap, Namespaces), "auto").expect("met");
            assert_eq!(bwrap.isolation, IsolationLevel::Namespaces);
            assert_eq!(bwrap.stage(), "bwrap");
        }
    }

    /// Das Sandbox-Profil verlangt, die Konfiguration wählt; widersprechen
    /// sie sich, startet nichts (`SANDBOX_036`).
    #[test]
    fn a_profile_that_requires_another_backend_refuses_the_start() {
        use humanitl_config::MinIsolation::Policy;
        use humanitl_config::SandboxBackendName::{Auto, Podman};
        // Ein Profil, das Podman verlangt, bei der Vorgabe `auto`; mit Knopf
        // zu Podman, denn seine gemeinsame Stufe ist zugelassen (HUM-448).
        let err = choose_backend(&sandbox(Auto, Policy), "podman").expect_err("requires");
        assert_eq!(err.code.as_str(), "SANDBOX_036", "{}", err.why);
        assert_eq!(
            err.fix,
            Some(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: PODMAN_BACKEND.to_owned(),
            })
        );
        // Ein Profil, das bwrap verlangt, bei der Wahl `podman`.
        if cfg!(not(target_os = "macos")) {
            let err = choose_backend(&sandbox(Podman, Policy), "bwrap").expect_err("requires");
            assert_eq!(err.code.as_str(), "SANDBOX_036", "{}", err.why);
            assert_eq!(
                err.fix,
                Some(FixAction::ChangeSetting {
                    key: "sandbox.backend".to_owned(),
                    value: "bwrap".to_owned(),
                })
            );
            choose_backend(&sandbox(Auto, Policy), "bwrap").expect("bwrap is auto here");
        }
        // Ein Profil, das das VM-Backend verlangt: ohne Knopf, denn es ist
        // nicht zugelassen, und der Knopf führte in `SANDBOX_048`.
        let err = choose_backend(&sandbox(Auto, Policy), VM_BACKEND).expect_err("requires");
        assert_eq!(err.code.as_str(), "SANDBOX_036", "{}", err.why);
        assert_eq!(err.fix, None);
        let err = choose_backend(&sandbox(Auto, Policy), "docker").expect_err("unknown");
        assert_eq!(err.code.as_str(), "CONFIG_003", "{}", err.why);
    }

    /// Ein Name des Schemas, den dieser Build nicht hat, ist `SANDBOX_034`
    /// und nie das Backend des Systems.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn seatbelt_outside_macos_is_not_available_and_does_not_fall_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        for refused in [
            Backend::detect(
                SEATBELT_BACKEND,
                without_bwrap(dir.path()),
                &SandboxRef::default(),
            ),
            Backend::fallback(
                SEATBELT_BACKEND,
                without_bwrap(dir.path()),
                &SandboxRef::default(),
            ),
        ] {
            let err = refused.expect_err("no seatbelt here");
            assert_eq!(err.code.as_str(), "SANDBOX_034", "{}", err.why);
            assert!(err.fix.is_some());
        }
    }

    /// Die Stufe eines Namens ist die des gebauten Backends, und die
    /// Untergrenze der Konfiguration hat dieselben drei Namen.
    #[test]
    fn the_isolation_of_a_name_is_that_of_its_backend() {
        let dir = tempfile::tempdir().expect("tempdir");
        let built = Backend::fallback(
            crate::AUTO_BACKEND,
            without_bwrap(dir.path()),
            &SandboxRef::default(),
        )
        .expect("auto");
        assert_eq!(isolation_of(built.name(), None), built.isolation());
        assert_eq!(
            isolation_of(PODMAN_BACKEND, Some(PodmanRuntime::Crun)),
            IsolationLevel::Namespaces
        );
        assert_eq!(isolation_of(VM_BACKEND, None), IsolationLevel::Kernel);
        for floor in humanitl_config::MinIsolation::ALL {
            assert_eq!(IsolationLevel::from(floor).as_str(), floor.as_str());
        }
    }
}
