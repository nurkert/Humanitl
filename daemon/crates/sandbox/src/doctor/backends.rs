//! Die Zeilen des Doctors je Sandbox-Backend (HUM-436).
//!
//! Mit zwei Backends fragt `humanitl doctor` jedes, das es auf dieser Art
//! System geben kann ([`LISTED_BACKENDS`]), und nennt je Backend, ob es
//! nutzbar ist und was fehlt. Jede Zeile trägt `ok`, `warn` oder `fail`, und
//! jede, die nicht `ok` ist, einen Befund mit `why` und `fix`.
//!
//! # Eine zugelassene Stufe
//!
//! - `admitted`: `ok`.
//! - `available`: das Ergebnis von [`Backend::detect`], dem Weg jedes
//!   Starts, samt Programm und Fassung.
//! - `probe`: nur nach einem Probestart. Auf Wunsch ([`ProbeMode::Start`])
//!   immer ein neuer; sonst aus dem Zwischenspeicher, solange sich keine
//!   Eingabe des Probestarts geändert hat und die gesetzten Ressourcen auf
//!   diesen Rechner passen ([`super::ProbeCache`], HUM-506). `DOCTOR_014`,
//!   wenn er scheiterte.
//!
//! # Eine Stufe, die noch nicht zugelassen ist
//!
//! - `admitted`: `warn` mit `SANDBOX_048`.
//! - Die Voraussetzungen, die sich ohne einen Start des Programms prüfen
//!   lassen, etwa `program`, `subordinate_ids` und `selinux`; welche das
//!   sind, weiß der Adapter (`crate::backend::static_checks`).
//! - `probe`, nur mit [`ProbeMode::Start`]: `warn` mit `SANDBOX_048`. Es
//!   startet nichts, nicht einmal das Programm, um seine Fassung zu lesen.
//!
//! Der Doctor ruft dafür nie `Backend::detect_unadmitted`; im ausgelieferten
//! Code gibt es die Funktion gar nicht (HUM-431).
//!
//! # Was nicht gewählt ist, färbt nichts rot
//!
//! Die Zeilen eines Backends, das `sandbox.backend` nicht wählt, sind
//! höchstens `warn`: Was ihm fehlt, hält keinen Start auf. Rot wird eine
//! Zeile nur in der Gruppe des gewählten Backends.

use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Instant;

use humanitl_config::{HostResources, Paths, SandboxRef};
use humanitl_core::diagnostics::codes::{DOCTOR_014, DOCTOR_015};
use humanitl_core::{Diagnostic, FixAction, Severity};

use super::probe_cache::{ProbeCache, ProbeKey};
use super::probe_start::{ProbeOutcome, probe_start};
use super::{CheckOutcome, DIAGNOSTICS_URL};
use crate::backend::{
    Backend, admitted, backend_name, beyond_host, resolve, runtime_of, search_files,
    stage as stage_name, static_checks,
};
use crate::backend_list::LISTED_BACKENDS;
use crate::backend_search::{
    BackendSearches, Ticket, Waited, fingerprint, pending_reason, search_key, searched_settings,
};

/// Die Zeilen in der Gruppe eines Backends.
///
/// Die Kurznamen sind Teil des Vertrags: Sie stehen als `DoctorCheck.id` in
/// `DoctorBackend.checks`, und die Oberfläche sucht ihre Überschrift danach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BackendCheckId {
    /// Ist die Stufe zugelassen (`SANDBOX_048`)?
    Admitted,
    /// Besteht das Backend die Suche vor jedem Start?
    Available,
    /// Gibt es das Programm? Nur für eine Stufe, die nicht zugelassen ist.
    Program,
    /// Hat das Konto untergeordnete IDs? Nur für eine Stufe, die nicht
    /// zugelassen ist.
    SubordinateIds,
    /// Erzwingt `SELinux` Etiketten? Nur für eine Stufe, die nicht zugelassen
    /// ist.
    Selinux,
    /// Hat der Probestart bestanden?
    Probe,
    /// Kernel-Stufe (HUM-444): Ist `/dev/kvm` nutzbar?
    Kvm,
    /// Kernel-Stufe: Gibt es den Emulator der Maschine mit ihrem Maschinentyp?
    Emulator,
    /// Kernel-Stufe: Gibt es den Dienst der Dateisysteme des Gasts?
    FileDaemon,
    /// Kernel-Stufe: Gibt es das vsock-Gerät der Tür?
    VsockDevice,
    /// Kernel-Stufe: Gibt es bubblewrap mit `--bind-fd`?
    Bubblewrap,
    /// Kernel-Stufe: Ist das Kernel-Abbild lesbar?
    KernelImage,
    /// Kernel-Stufe: Hat der Kernel virtiofs eingebaut?
    VirtioFs,
    /// Kernel-Stufe: Gibt es das Verzeichnis seiner Module?
    KernelModules,
}

impl BackendCheckId {
    /// Der Kurzname in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Admitted => "admitted",
            Self::Available => "available",
            Self::Program => "program",
            Self::SubordinateIds => "subordinate_ids",
            Self::Selinux => "selinux",
            Self::Probe => "probe",
            Self::Kvm => "kvm",
            Self::Emulator => "emulator",
            Self::FileDaemon => "file_daemon",
            Self::VsockDevice => "vsock_device",
            Self::Bubblewrap => "bubblewrap",
            Self::KernelImage => "kernel",
            Self::VirtioFs => "virtio_fs",
            Self::KernelModules => "modules",
        }
    }
}

impl std::fmt::Display for BackendCheckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Was ein Adapter über eine Voraussetzung meldet, die sich ohne Start
/// prüfen lässt (HUM-436).
///
/// `I` ist die Art, wie der Adapter seine Zeilen benennt; der Doctor nimmt
/// sie als [`BackendCheckId`]. Eine Zeile, die eine andere braucht, um
/// überhaupt prüfen zu können, wiederholt deren Befund nie, sondern ist
/// [`StaticLine::Unchecked`] (HUM-495): Keine Zeile meldet einen Fehler, den
/// sie nicht geprüft hat.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StaticLine<I = BackendCheckId> {
    /// Geprüft: der Beleg oder der Befund.
    Checked(Result<String, Diagnostic>),
    /// Nicht prüfbar, weil die Voraussetzung `after` fehlt, die diese Prüfung
    /// braucht; deren Zeile nennt den Befund. `after` ist immer eine Zeile
    /// derselben Gruppe, die selbst gescheitert ist
    /// (`Checked(Err(_))`); sonst wäre der Verweis falsch. Bisher meldet nur
    /// die Kernel-Stufe so eine Zeile, die es nur unter Linux auf x86-64 gibt.
    #[cfg_attr(
        not(all(target_os = "linux", target_arch = "x86_64")),
        allow(dead_code)
    )]
    Unchecked {
        /// Die Zeile, die gescheitert ist.
        after: I,
        /// Was diese Prüfung von ihr braucht, als Satzteil, etwa
        /// `the kernel image`.
        needs: &'static str,
    },
}

impl<I> From<Result<String, Diagnostic>> for StaticLine<I> {
    fn from(result: Result<String, Diagnostic>) -> Self {
        Self::Checked(result)
    }
}

/// Die Zeilen eines Backends.
#[derive(Debug, Clone, PartialEq)]
pub struct BackendGroup {
    /// Der Name in `sandbox.backend`, zum Beispiel `bwrap`.
    pub name: &'static str,
    /// Die Stufe: das Backend, mit seiner Laufzeit, falls es eine hat.
    pub stage: String,
    /// Ob `sandbox.backend` dieses Backend wählt (`auto` aufgelöst).
    pub selected: bool,
    /// Die Zeilen, in Anzeigereihenfolge.
    pub checks: Vec<CheckOutcome<BackendCheckId>>,
}

impl BackendGroup {
    /// Die Zeile mit dieser Kennung.
    #[must_use]
    pub fn get(&self, id: BackendCheckId) -> Option<&CheckOutcome<BackendCheckId>> {
        self.checks.iter().find(|check| check.id() == id)
    }
}

/// Ob der Doctor einen Probestart macht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeMode {
    /// Keiner. Ein gelungener Probestart aus dem Zwischenspeicher steht
    /// trotzdem da, solange sich keine seiner Eingaben geändert hat.
    Cached,
    /// Auf Wunsch: Probestart je zugelassenem, verfügbarem Backend, mit
    /// diesem Shim ([`super::locate_shim`]). Immer ein neuer; der
    /// Zwischenspeicher wird nur geschrieben, nie gelesen (HUM-506).
    Start {
        /// Der Shim auf dem Host.
        shim: PathBuf,
    },
}

/// Die Gruppen aller Backends dieser Art System, in der Reihenfolge von
/// [`LISTED_BACKENDS`].
///
/// `sandbox` ist die aufgelöste Gruppe `sandbox` ohne Projekt. Blockiert:
/// Die Suche startet kurze Programme, und [`ProbeMode::Start`] startet je
/// zugelassenem Backend eine Sandbox. Wartet auf jede Suche; die Backends
/// suchen nebeneinander ([`backend_groups_within`]).
#[must_use]
pub fn backend_groups(sandbox: &SandboxRef, paths: &Paths, mode: &ProbeMode) -> Vec<BackendGroup> {
    backend_groups_within(sandbox, paths, mode, &BackendSearches::new(), None)
}

/// Wie [`backend_groups`], mit dem Gedächtnis `searches` und bis `deadline`
/// (HUM-502).
///
/// Jedes Backend sucht in einem eigenen Thread, oder die Antwort nimmt eine
/// Suche aus `searches`, die läuft oder noch gilt
/// ([`crate::backend_search`]). Läuft die Suche eines Backends bei
/// `deadline` noch, besteht seine Gruppe aus `admitted` und einer Zeile
/// `available` ohne Messung (`warn`, `DOCTOR_012`, „still being checked"),
/// auch unter dem gewählten Backend; die übrigen Gruppen sind vollständig.
/// Die Frist gilt der Suche: Einen Probestart auf Wunsch macht die Antwort
/// danach ganz, wie bisher.
/// Mit [`ProbeMode::Start`] nimmt sie keine gemerkte Suche, sondern sucht
/// neu.
#[must_use]
pub fn backend_groups_within(
    sandbox: &SandboxRef,
    paths: &Paths,
    mode: &ProbeMode,
    searches: &BackendSearches,
    deadline: Option<Instant>,
) -> Vec<BackendGroup> {
    let cache = ProbeCache::of(paths);
    // Ein Probestart ist ein ausdrücklicher Wunsch: Er nimmt keine gemerkte
    // Suche, damit eine eben getroffene Abhilfe zählt.
    if matches!(mode, ProbeMode::Start { .. }) {
        for &name in LISTED_BACKENDS {
            searches.invalidate(name, sandbox, paths);
        }
    }
    // Erst jede Suche starten, dann warten: So laufen sie nebeneinander, und
    // die Frist gilt für alle zusammen.
    let started: Vec<(&'static str, Ticket<DoctorSearch>)> = LISTED_BACKENDS
        .iter()
        .map(|&name| (name, start_search(name, sandbox, paths, searches)))
        .collect();
    started
        .into_iter()
        .map(|(name, ticket)| finish_group(name, sandbox, mode, &cache, ticket.wait(deadline)))
        .collect()
}

/// Was die Suche des Doctors über ein Backend ergab; gemerkt in
/// [`BackendSearches`], ohne den Probestart, den jede Antwort selbst
/// nachsieht.
#[derive(Debug, Clone)]
pub(crate) enum DoctorSearch {
    /// Die Stufe ist nicht zugelassen: die Ablehnung und die
    /// Voraussetzungen, die sich ohne Start prüfen lassen.
    Unadmitted {
        refused: Diagnostic,
        statics: Vec<CheckOutcome<BackendCheckId>>,
    },
    /// Die Suche vor jedem Start hat dieses Backend gefunden.
    Found(Backend),
    /// Die Suche scheiterte; dazu je Voraussetzung eine Zeile (HUM-488).
    Missing {
        missing: Diagnostic,
        statics: Vec<CheckOutcome<BackendCheckId>>,
    },
}

/// Startet die Suche des Doctors für dieses Backend über das Gedächtnis,
/// oder schließt sich der an, die läuft oder gilt. Die Suche vor jedem Start
/// darin teilt sie mit der Liste der Backends.
fn start_search(
    name: &'static str,
    sandbox: &SandboxRef,
    paths: &Paths,
    searches: &BackendSearches,
) -> Ticket<DoctorSearch> {
    let (owned_sandbox, owned_paths, detector) = (sandbox.clone(), paths.clone(), searches.clone());
    searches.doctor.start(
        search_key(name, sandbox, paths),
        &search_files(name, sandbox, paths),
        move || {
            // Bricht die Suche vor jedem Start ohne Ergebnis ab, endet auch
            // diese ohne: Die Antwort sagt „wird geprüft", und die nächste
            // Frage sucht neu, statt 30 s lang einen Befund zu zeigen, den
            // niemand gemessen hat.
            let detected = match detector
                .detect(name, &owned_sandbox, &owned_paths)
                .wait(None)
            {
                Waited::Done(found) => found,
                Waited::Pending(_) => return None,
            };
            Some(search(name, &owned_sandbox, &owned_paths, || detected))
        },
    )
}

/// Die Suche des Doctors: vor der Zulassung nur die Voraussetzungen ohne
/// Start, danach die Suche vor jedem Start (`detected`), und scheitert sie,
/// dazu je Voraussetzung eine Zeile.
fn search(
    name: &'static str,
    sandbox: &SandboxRef,
    paths: &Paths,
    detected: impl FnOnce() -> Result<Backend, Diagnostic>,
) -> DoctorSearch {
    match admitted(name, runtime_of(name, sandbox)) {
        Err(refused) => DoctorSearch::Unadmitted {
            refused,
            statics: static_lines(name, paths, sandbox),
        },
        Ok(()) => match detected() {
            Ok(backend) => DoctorSearch::Found(backend),
            // Die Suche nennt nur die erste fehlende Voraussetzung; doctor
            // sagt alles, was fehlt, je Voraussetzung eine Zeile (HUM-488),
            // wie vor der Zulassung.
            Err(missing) => DoctorSearch::Missing {
                missing,
                statics: static_lines(name, paths, sandbox),
            },
        },
    }
}

/// Die Gruppe eines Backends, ohne Gedächtnis und ohne Frist.
#[cfg(test)]
pub(crate) fn group(
    name: &'static str,
    sandbox: &SandboxRef,
    paths: &Paths,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> BackendGroup {
    let searched = search(name, sandbox, paths, || {
        crate::backend::detect_in_build(name, paths.clone(), sandbox)
    });
    finish_group(name, sandbox, mode, cache, Waited::Done(searched))
}

/// Die Gruppe eines Backends aus dem, was seine Suche ergab oder dass sie
/// noch läuft.
fn finish_group(
    name: &'static str,
    sandbox: &SandboxRef,
    mode: &ProbeMode,
    cache: &ProbeCache,
    searched: Waited<DoctorSearch>,
) -> BackendGroup {
    let runtime = runtime_of(name, sandbox);
    let stage = stage_name(name, runtime).into_owned();
    let selected = resolve(backend_name(sandbox.backend)).ok() == Some(name);
    let mut checks = match searched {
        Waited::Done(DoctorSearch::Unadmitted { refused, statics }) => {
            unadmitted_lines(&stage, refused, statics, mode)
        }
        Waited::Done(DoctorSearch::Found(backend)) => {
            found_lines(name, &stage, sandbox, &backend, mode, cache)
        }
        Waited::Done(DoctorSearch::Missing { missing, statics }) => {
            missing_lines(&stage, &missing, statics, mode)
        }
        Waited::Pending(running) => pending_lines(name, &stage, admitted(name, runtime), running),
    };
    if !selected {
        checks = checks.into_iter().map(CheckOutcome::at_most_warn).collect();
    }
    BackendGroup {
        name,
        stage,
        selected,
        checks,
    }
}

/// Die Zeilen eines Backends, dessen Suche bei Ablauf der Frist noch läuft:
/// die Zulassung, die nichts kostet, und `available` ohne Messung mit dem
/// Befehl, der ohne Frist auf das Ergebnis wartet. Nie `fail`: Nachgesehen
/// hat noch niemand.
fn pending_lines(
    name: &'static str,
    stage: &str,
    admission: Result<(), Diagnostic>,
    running: std::time::Duration,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let admission = admission.map_or_else(
        |refused| {
            CheckOutcome::warn(
                BackendCheckId::Admitted,
                format!("{stage} is not admitted yet"),
                refused,
            )
        },
        |()| CheckOutcome::ok(BackendCheckId::Admitted, format!("{stage} is admitted")),
    );
    vec![
        admission,
        CheckOutcome::unmeasured(
            BackendCheckId::Available,
            pending_reason(name, running),
            &["humanitl", "doctor"],
        ),
    ]
}

/// Die Zeilen einer Stufe, die nicht zugelassen ist: `SANDBOX_048`, die
/// Voraussetzungen ohne Start, und mit [`ProbeMode::Start`] eine Zeile
/// `probe`, die sagt, dass nichts startete.
fn unadmitted_lines(
    stage: &str,
    refused: Diagnostic,
    statics: Vec<CheckOutcome<BackendCheckId>>,
    mode: &ProbeMode,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let mut checks = vec![CheckOutcome::warn(
        BackendCheckId::Admitted,
        format!("{stage} is not admitted yet"),
        refused.clone(),
    )];
    checks.extend(statics);
    if matches!(mode, ProbeMode::Start { .. }) {
        checks.push(CheckOutcome::warn(
            BackendCheckId::Probe,
            format!("not started: {stage} is not admitted yet"),
            refused,
        ));
    }
    checks
}

/// Je Voraussetzung einer Stufe eine Zeile, ohne etwas zu starten.
fn static_lines(
    name: &'static str,
    paths: &Paths,
    sandbox: &SandboxRef,
) -> Vec<CheckOutcome<BackendCheckId>> {
    static_checks(name, paths, sandbox)
        .into_iter()
        .map(|(id, line)| static_outcome(id, line))
        .collect()
}

/// Die Zeile des Doctors zu dem, was ein Adapter über `id` meldet. Eine
/// Prüfung, die eine fehlende Voraussetzung brauchte, ist ohne Messung
/// (`warn`, `DOCTOR_012`) und verweist auf deren Zeile (HUM-495); den Befund
/// dort wiederholt sie nicht. Sie nennt, was ihr fehlt, und dass zuerst die
/// Zeile `after` behoben sein muss; der Vorschlag ist, wie bei jeder Zeile
/// ohne Messung, der Befehl, der sie dann misst. Das gilt nur, solange
/// `after` eine gescheiterte Zeile ist ([`StaticLine::Unchecked`]).
fn static_outcome(id: BackendCheckId, line: StaticLine) -> CheckOutcome<BackendCheckId> {
    match line {
        StaticLine::Checked(Ok(evidence)) => CheckOutcome::ok(id, evidence),
        StaticLine::Checked(Err(missing)) => {
            CheckOutcome::fail(id, format!("missing: {}", missing.code), missing)
        }
        StaticLine::Unchecked { after, needs } => CheckOutcome::unmeasured(
            id,
            format!(
                "it needs {needs}, and the line {after} reports it missing; fix the line \
                 {after} first, then run humanitl doctor again"
            ),
            &["humanitl", "doctor"],
        ),
    }
}

/// Die Zeilen einer zugelassenen Stufe, deren Suche bestand: die Suche samt
/// Programm und Fassung und der Probestart, gespeichert oder auf Wunsch neu.
fn found_lines(
    name: &'static str,
    stage: &str,
    sandbox: &SandboxRef,
    backend: &Backend,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let key = probe_key(name, stage, sandbox, backend);
    let mut checks = vec![
        CheckOutcome::ok(BackendCheckId::Admitted, format!("{stage} is admitted")),
        CheckOutcome::ok(
            BackendCheckId::Available,
            format!("{} {}", backend.program().display(), key.tool_version)
                .trim_end()
                .to_owned(),
        ),
    ];
    checks.extend(probe_line(name, sandbox, backend, &key, mode, cache));
    checks
}

/// Die Zeilen einer zugelassenen Stufe, deren Suche scheiterte: der Befund
/// der Suche und je Voraussetzung eine Zeile; der Probestart auf Wunsch
/// startet nichts.
fn missing_lines(
    stage: &str,
    missing: &Diagnostic,
    statics: Vec<CheckOutcome<BackendCheckId>>,
    mode: &ProbeMode,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let mut checks = vec![
        CheckOutcome::ok(BackendCheckId::Admitted, format!("{stage} is admitted")),
        CheckOutcome::fail(
            BackendCheckId::Available,
            format!("not usable: {}", missing.code),
            missing.clone(),
        ),
    ];
    checks.extend(statics);
    if matches!(mode, ProbeMode::Start { .. }) {
        checks.push(CheckOutcome::unmeasured(
            BackendCheckId::Probe,
            format!("{stage} is not usable here ({})", missing.code),
            &["humanitl", "sandbox", "backends"],
        ));
    }
    checks
}

/// Die Zeile des Probestarts: auf Wunsch aus einem neuen Start, sonst aus
/// dem Zwischenspeicher oder keine.
///
/// Ein Probestart auf Wunsch liest den Zwischenspeicher nie (HUM-506): Wer
/// `--probe` sagt, will wissen, ob das Backend jetzt startet, auch wenn sich
/// eine Eingabe geändert hat, die kein Abdruck fängt.
fn probe_line(
    name: &'static str,
    sandbox: &SandboxRef,
    backend: &Backend,
    key: &ProbeKey,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> Option<CheckOutcome<BackendCheckId>> {
    let ProbeMode::Start { shim } = mode else {
        return cached_probe(name, sandbox, key, cache, HostResources::probe()).map(|evidence| {
            CheckOutcome::ok(BackendCheckId::Probe, format!("{evidence} (cached)"))
        });
    };
    Some(match probe_start(backend, shim) {
        ProbeOutcome::Passed(evidence) => {
            // Ein Fehler beim Merken kostet nur den nächsten Probestart.
            let _ = cache.store(key, &evidence);
            CheckOutcome::ok(BackendCheckId::Probe, evidence)
        }
        ProbeOutcome::NotStarted(inner) => CheckOutcome::fail(
            BackendCheckId::Probe,
            format!("not started: {}", inner.code),
            probe_failed(&key.stage, "did not start", Some(&inner)),
        ),
        ProbeOutcome::Incomplete { what, inner } => CheckOutcome::fail(
            BackendCheckId::Probe,
            format!("incomplete: {what}"),
            probe_failed(&key.stage, &format!("started, and {what}"), inner.as_ref()),
        ),
        ProbeOutcome::Failed {
            check,
            evidence,
            diagnostic,
        } => CheckOutcome::fail(
            BackendCheckId::Probe,
            format!("{check}: {evidence}"),
            probe_failed(
                &key.stage,
                &format!("started, and the guarantee {check} did not hold ({evidence})"),
                diagnostic.as_ref(),
            ),
        ),
    })
}

/// `DOCTOR_014`: Der Probestart ist gescheitert; unter der Kernel-Stufe
/// `DOCTOR_015` (HUM-444). Code und Grund des inneren Befunds stehen im
/// `why`, sein Vorschlag wird übernommen.
pub(crate) fn probe_failed(stage: &str, what: &str, inner: Option<&Diagnostic>) -> Diagnostic {
    let mut why = format!("the probe start of an empty sandbox under {stage} {what}");
    if let Some(inner) = inner {
        let _ = write!(why, "; {}: {}", inner.code, inner.why);
    }
    let (code, anchor) = if stage == crate::VM_BACKEND {
        (DOCTOR_015, "#doctor_015")
    } else {
        (DOCTOR_014, "#doctor_014")
    };
    Diagnostic::builder(code, Severity::Blocking)
        .why(why)
        .fix(
            inner
                .and_then(|inner| inner.fix.clone())
                .unwrap_or_else(|| FixAction::OpenUrl(format!("{DIAGNOSTICS_URL}{anchor}"))),
        )
        .build()
}

/// Der Beleg eines gespeicherten Probestarts mit genau diesem Schlüssel,
/// wenn die gesetzten Ressourcen der Gruppe des Backends auf `host` passen.
///
/// Der Plan jedes Starts prüft sie gegen den Rechner, bevor er etwas startet
/// (HUM-501). Ein Probestart, der bei mehr Speicher oder mehr CPUs bestand,
/// etwa vor einem `taskset`, sagt über einen Start jetzt nichts; dann gilt
/// kein Eintrag, und erst `--probe` sagt, woran es liegt.
pub(crate) fn cached_probe(
    name: &'static str,
    sandbox: &SandboxRef,
    key: &ProbeKey,
    cache: &ProbeCache,
    host: HostResources,
) -> Option<String> {
    if beyond_host(name, sandbox, host).is_some() {
        return None;
    }
    cache.lookup(key)
}

/// Der Schlüssel des Zwischenspeichers für dieses Backend.
fn probe_key(name: &'static str, stage: &str, sandbox: &SandboxRef, backend: &Backend) -> ProbeKey {
    ProbeKey {
        stage: stage.to_owned(),
        program: backend.program().display().to_string(),
        tool_version: backend.tool_version(),
        inputs: probe_inputs(name, sandbox, backend.paths()),
        humanitl_version: env!("CARGO_PKG_VERSION").to_owned(),
        machine: super::probe_cache::machine_identity(),
    }
}

/// Der Abdruck der Eingaben eines Probestarts außer Programm und Fassung
/// (HUM-506), als SHA-256 in Hex: die Einstellungen der Backends in der
/// Gruppe `sandbox` (darunter die gesetzten Ressourcen, Gastkernel und
/// Modulverzeichnis), die Pfade samt Umgebung, der temporäre Ordner und der
/// Abdruck jeder Datei, von der die Suche des
/// Backends abhängt ([`search_files`]). Unter der Kernel-Stufe sind das
/// Emulator, Dateidienst, vsock-Gerät, bubblewrap, Kernel-Abbild samt
/// Konfiguration, Modulverzeichnis und seine Listen; ein Update eines davon
/// ändert Inode, Größe oder Zeit, und der gespeicherte Probestart gilt nicht
/// mehr.
///
/// Gehasht, weil die Datei des Zwischenspeichers nur einen Vergleich
/// braucht, und weil die Umgebung des Daemons Werte tragen kann, die nicht
/// auf die Platte gehören.
pub(crate) fn probe_inputs(name: &'static str, sandbox: &SandboxRef, paths: &Paths) -> String {
    // Die Pfade samt der Umgebung des Daemons: Aus ihr folgen der
    // Laufzeitordner, das Proxy-Verzeichnis und der `PATH`, mit denen ein
    // Start plant; dazu der temporäre Ordner des Probestarts. Nur der Hash
    // landet in der Datei, nie ein Wert daraus.
    let text = format!(
        "{:?}\n{paths:?}\n{}\n{}",
        searched_settings(sandbox),
        std::env::temp_dir().display(),
        fingerprint(&search_files(name, sandbox, paths))
    );
    super::probe_cache::sha256_hex(text.as_bytes())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{Env, Paths, SandboxBackendName, SandboxRef};

    use super::{BackendCheckId, ProbeMode, group};
    use crate::doctor::{CheckStatus, ProbeCache};

    /// Ein Ordner mit `bwrap` und `humanitl-shim` im `PATH`, die jeden
    /// Aufruf in die Datei `calls` schreiben, und sonst keinem Programm.
    fn counting_programs(dir: &std::path::Path) -> (Paths, std::path::PathBuf) {
        let bin = dir.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let calls = dir.join("calls");
        for name in ["bwrap", "humanitl-shim"] {
            crate::test_support::write_program(
                &bin.join(name),
                &format!(
                    "#!/bin/sh\necho \"$0 $@\" >> '{}'\nexit 0\n",
                    calls.display()
                ),
            );
        }
        let paths = Paths::new(Env::from_pairs([
            ("HOME", dir.to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
        ]));
        (paths, calls)
    }

    /// Die Gruppe `sandbox`, die das VM-Backend der Kernel-Stufe wählt.
    fn kernel_level() -> SandboxRef {
        SandboxRef {
            backend: SandboxBackendName::Qemu,
            ..SandboxRef::default()
        }
    }

    /// Seit HUM-449 ist das VM-Backend der Kernel-Stufe zugelassen: Seine
    /// Gruppe sucht es wie jeder Start (`available`), und fehlt eine
    /// Voraussetzung, steht ihr `SANDBOX_045` da, nie `SANDBOX_048`. Der
    /// Probestart auf Wunsch startet dann nichts, keinen Shim und keine
    /// Sandbox, und merkt sich nichts.
    #[test]
    fn the_admitted_kernel_level_is_searched_like_a_start_and_probes_nothing_it_lacks() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, calls) = counting_programs(dir.path());
        let sandbox = kernel_level();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let mode = ProbeMode::Start {
            shim: dir.path().join("bin").join("humanitl-shim"),
        };

        let group = group(crate::VM_BACKEND, &sandbox, &paths, &mode, &cache);

        assert!(group.selected);
        assert_eq!(group.stage, crate::VM_BACKEND);
        let admitted = group.get(BackendCheckId::Admitted).expect("admitted");
        assert_eq!(admitted.status(), CheckStatus::Ok, "{admitted:?}");
        let available = group.get(BackendCheckId::Available).expect("available");
        assert_eq!(available.status(), CheckStatus::Fail, "{available:?}");
        let missing = if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            "SANDBOX_045"
        } else {
            "SANDBOX_034"
        };
        assert_eq!(
            available.diagnostic().map(|d| d.code.as_str()),
            Some(missing)
        );
        let probe = group.get(BackendCheckId::Probe).expect("probe");
        assert_eq!(probe.status(), CheckStatus::Warn, "{probe:?}");
        assert!(probe.evidence().contains(missing), "{probe:?}");
        // Alles, was fehlt, je Voraussetzung eine Zeile (HUM-488), nicht nur
        // die erste: jede Voraussetzung steht da, der fehlende Emulator als
        // Fehler. Mutationsbeweis: ohne die Zeilen fehlt jede davon.
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            for id in [
                BackendCheckId::Kvm,
                BackendCheckId::Emulator,
                BackendCheckId::FileDaemon,
                BackendCheckId::VsockDevice,
                BackendCheckId::Bubblewrap,
                BackendCheckId::KernelImage,
                BackendCheckId::VirtioFs,
                BackendCheckId::KernelModules,
            ] {
                assert!(group.get(id).is_some(), "{id:?}: {:?}", group.checks);
            }
            let emulator = group.get(BackendCheckId::Emulator).expect("emulator");
            assert_eq!(emulator.status(), CheckStatus::Fail, "{emulator:?}");
        }
        for line in &group.checks {
            assert_ne!(
                line.diagnostic().map(|d| d.code.as_str()),
                Some("SANDBOX_048"),
                "{line:?}"
            );
        }
        let called = std::fs::read_to_string(&calls).unwrap_or_default();
        assert!(!called.contains("humanitl-shim"), "the shim ran: {called}");
        assert!(!cache.path().exists(), "nothing was probed, nothing cached");
    }

    /// Eine Liste der Zulassung ohne das VM-Backend der Kernel-Stufe
    /// ([`crate::test_support::with_admitted`]).
    const WITHOUT_KERNEL_LEVEL: &[&str] = &[crate::BWRAP_BACKEND, crate::SEATBELT_BACKEND];

    /// Für eine Stufe vor ihrer Zulassung liefert `doctor --probe`
    /// `SANDBOX_048` und startet keinen Prozess. Seit HUM-449 ist jede Stufe
    /// dieses Builds zugelassen; der Test nimmt deshalb die Kernel-Stufe aus
    /// der Liste (HUM-493) und geht den Weg jeder Gruppe, mit der Suche:
    /// Jedes Programm im `PATH` zählt seinen Aufruf in eine Datei, und
    /// gestartet wird nichts. Mutationsbeweis: Ohne den Zweig der
    /// Zulassung in `search` sucht die Gruppe das Backend, `available` steht
    /// da und `admitted` ist `ok`.
    #[test]
    fn before_admission_the_probe_is_refused_and_starts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, calls) = counting_programs(dir.path());
        let sandbox = kernel_level();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let mode = ProbeMode::Start {
            shim: dir.path().join("bin").join("humanitl-shim"),
        };

        let search = || group(crate::VM_BACKEND, &sandbox, &paths, &mode, &cache);
        let group = crate::test_support::with_admitted(WITHOUT_KERNEL_LEVEL, search);
        let checks = group.checks;

        assert!(!cache.path().exists(), "nothing was probed, nothing cached");
        let get = |id| checks.iter().find(|check| check.id() == id);
        for id in [BackendCheckId::Admitted, BackendCheckId::Probe] {
            let line = get(id).expect("the line is there");
            assert_eq!(line.status(), CheckStatus::Warn, "{line:?}");
            assert_eq!(
                line.diagnostic().map(|d| d.code.as_str()),
                Some("SANDBOX_048")
            );
        }
        assert!(get(BackendCheckId::Available).is_none());
        // Die Kernel-Stufe hat je Voraussetzung eine Zeile (HUM-444); die
        // Fähigkeiten eines Programms liest sie aus seiner Hilfe (HUM-464).
        // Eine Frage nach der Hilfe ist der einzige Aufruf, und gestartet wird
        // nichts: kein Shim, keine Sandbox. Genau eine Frage, an genau dieses
        // Programm; die übrigen Programme fehlen im `PATH` dieses Tests. Die
        // Datei muss es geben: Ohne sie belegte das Fehlen jedes Aufrufs
        // nichts über die Zeilen. Außerhalb von Linux auf x86-64 gibt es die
        // Kernel-Stufe nicht; dort fragt der Doctor kein Programm, und die
        // Datei entsteht nie.
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            let calls = std::fs::read_to_string(&calls).expect("bwrap was asked for its help");
            assert_eq!(
                calls.lines().collect::<Vec<_>>(),
                [format!(
                    "{} --help",
                    dir.path().join("bin").join("bwrap").display()
                )],
                "a process was started"
            );
            assert!(get(BackendCheckId::Bubblewrap).is_some());
            assert!(get(BackendCheckId::Kvm).is_some());
        } else {
            assert!(!calls.exists(), "a process was started");
        }
    }

    /// Läuft die Suche bei Ablauf der Frist noch, sagt die Gruppe trotzdem,
    /// ob die Stufe zugelassen ist: vor der Zulassung `warn` mit
    /// `SANDBOX_048`, danach `ok` (HUM-493). Mutationsbeweis: Ohne den Zweig
    /// der Ablehnung in `pending_lines` stünde vor der Zulassung `ok` da.
    #[test]
    fn a_pending_search_still_names_the_admission() {
        let dir = tempfile::tempdir().unwrap();
        let sandbox = kernel_level();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let pending = || {
            super::finish_group(
                crate::VM_BACKEND,
                &sandbox,
                &ProbeMode::Cached,
                &cache,
                crate::backend_search::Waited::Pending(std::time::Duration::from_secs(1)),
            )
        };

        let before = crate::test_support::with_admitted(WITHOUT_KERNEL_LEVEL, pending);
        let admitted = before.get(BackendCheckId::Admitted).expect("admitted");
        assert_eq!(admitted.status(), CheckStatus::Warn, "{admitted:?}");
        assert_eq!(
            admitted.diagnostic().map(|d| d.code.as_str()),
            Some("SANDBOX_048")
        );
        assert!(before.get(BackendCheckId::Available).is_some());

        let after = pending();
        let admitted = after.get(BackendCheckId::Admitted).expect("admitted");
        assert_eq!(admitted.status(), CheckStatus::Ok, "{admitted:?}");
    }

    /// HUM-502: Der Doctor merkt sich die Suche, aber ein Probestart auf
    /// Wunsch sucht neu, damit eine eben getroffene Abhilfe zählt. Jedes
    /// Programm zählt seinen Aufruf. Mutationsbeweis: ohne das Verwerfen
    /// bleibt die Zahl der Aufrufe beim Probestart gleich. Nur unter Linux:
    /// Unter macOS sucht keines der gelisteten Backends ein Programm im
    /// `PATH` des Tests, es gäbe nichts zu zählen.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_requested_probe_searches_again_and_a_plain_doctor_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let (paths, calls) = counting_programs(dir.path());
        let sandbox = humanitl_config::SandboxRef::default();
        let searches = crate::backend_search::BackendSearches::new();
        let count = || {
            std::fs::read_to_string(&calls)
                .unwrap_or_default()
                .lines()
                .count()
        };
        let run = |mode: &ProbeMode| {
            super::backend_groups_within(&sandbox, &paths, mode, &searches, None)
        };
        run(&ProbeMode::Cached);
        let asked = count();
        assert!(asked > 0);
        run(&ProbeMode::Cached);
        assert_eq!(count(), asked, "kept");
        run(&ProbeMode::Start {
            shim: dir.path().join("bin").join("humanitl-shim"),
        });
        assert!(count() > asked, "the probe searched again");
    }

    /// HUM-506: `--probe` startet immer neu, auch wenn ein gelungener
    /// Probestart mit genau diesem Schlüssel gespeichert ist; ohne `--probe`
    /// steht der gespeicherte da. Das Programm des Backends ist eine Attrappe,
    /// die sofort mit 1 endet, ohne den Shim (eine zweite Attrappe, die es
    /// gibt) je zu starten: Ein neuer Start ist so `fail` mit `SANDBOX_012`
    /// („endete, ohne den Befehl zu starten"), ein gespeicherter `ok`.
    ///
    /// Mutationsbeweis: Sieht `probe_line` wieder vor der Frage nach dem
    /// Modus im Zwischenspeicher nach, antwortet `--probe` mit „(cached)",
    /// und dieser Test fällt. Nur unter Linux: Dort startet ein Probestart
    /// das Programm des Backends.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_requested_probe_never_answers_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let program = bin.join("bwrap");
        crate::test_support::write_program(&program, "#!/bin/sh\nexit 1\n");
        let shim = bin.join("humanitl-shim");
        crate::test_support::write_program(&shim, "#!/bin/sh\nexit 0\n");
        let runtime = dir.path().join("run");
        std::fs::create_dir_all(&runtime).unwrap();
        let paths = Paths::new(Env::from_pairs([
            ("HOME", dir.path().to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
            ("XDG_RUNTIME_DIR", runtime.to_string_lossy().into_owned()),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").to_string_lossy().into_owned(),
            ),
        ]));
        let sandbox = SandboxRef::default();
        let backend = crate::Backend::from(crate::bwrap::BwrapBackend::unchecked(
            &program,
            crate::bwrap::MIN_BWRAP_VERSION,
            paths,
        ));
        let name = crate::backend::BWRAP_BACKEND;
        let key = super::probe_key(name, name, &sandbox, &backend);
        if key.machine.is_empty() {
            // Ohne Kennung des Rechners gilt kein Eintrag, auch kein
            // gespeicherter; dieser Test hätte nichts zu unterscheiden.
            eprintln!("skipped: this machine has no identity (machine_identity is empty)");
            return;
        }
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        cache.store(&key, "passed").unwrap();
        let line = |mode: &ProbeMode| {
            super::probe_line(name, &sandbox, &backend, &key, mode, &cache).expect("a probe line")
        };

        let cached = line(&ProbeMode::Cached);
        assert_eq!(cached.status(), CheckStatus::Ok, "{cached:?}");
        assert_eq!(cached.evidence(), "passed (cached)");

        // Wie der Start an der Attrappe scheitert, hängt von der Umgebung ab
        // (ein Runner meldet einen fehlenden Bericht, ein Desktop
        // `SANDBOX_012`); gezählt wird, dass er lief und nicht die
        // gespeicherte Antwort gab.
        let requested = line(&ProbeMode::Start { shim });
        assert_eq!(requested.status(), CheckStatus::Fail, "{requested:?}");
        assert!(!requested.evidence().contains("cached"), "{requested:?}");
    }

    /// HUM-506: Ein anderer Laufzeitordner, aus dem ein Start Proxy-Socket
    /// und Shim-Ordner plant, ändert den Abdruck, auch wenn sonst alles
    /// gleich ist. In der Datei steht davon nur der Hash.
    ///
    /// Mutationsbeweis: ohne die Pfade in `probe_inputs` bleibt der andere
    /// Laufzeitordner unbemerkt.
    #[test]
    fn the_probe_inputs_follow_the_runtime_dir() {
        let dir = tempfile::tempdir().unwrap();
        let paths = |runtime: &str| {
            Paths::new(Env::from_pairs([
                ("HOME", dir.path().to_string_lossy().into_owned()),
                (
                    "XDG_RUNTIME_DIR",
                    dir.path().join(runtime).to_string_lossy().into_owned(),
                ),
            ]))
        };
        let sandbox = SandboxRef::default();
        let name = crate::backend::BWRAP_BACKEND;
        let first = super::probe_inputs(name, &sandbox, &paths("run-a"));
        assert_eq!(first, super::probe_inputs(name, &sandbox, &paths("run-a")));
        assert_ne!(first, super::probe_inputs(name, &sandbox, &paths("run-b")));
        assert!(!first.contains("run-a"), "only a hash: {first}");
    }

    /// HUM-506: Der Abdruck der Eingaben ändert sich, wenn das Programm
    /// eines Backends ersetzt wird oder neu im `PATH` auftaucht, auch bei
    /// gleicher Fassung. Dass eine Einstellung ihn ändert, prüft der Test
    /// des Adapters der Kernel-Stufe.
    ///
    /// Mutationsbeweis: ohne `fingerprint` in `probe_inputs` bleibt das
    /// ersetzte Programm unbemerkt.
    #[test]
    fn the_probe_inputs_follow_the_program_file() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let paths = Paths::new(Env::from_pairs([
            ("HOME", dir.path().to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
        ]));
        let sandbox = SandboxRef::default();
        let name = crate::backend::BWRAP_BACKEND;
        let inputs = |sandbox: &SandboxRef| super::probe_inputs(name, sandbox, &paths);

        let absent = inputs(&sandbox);
        let program = bin.join(name);
        std::fs::write(&program, b"#!/bin/sh\n").unwrap();
        let present = inputs(&sandbox);
        assert_ne!(absent, present, "the program appeared");
        assert_eq!(present, inputs(&sandbox), "nothing changed");

        let replacement = bin.join("replacement");
        std::fs::write(&replacement, b"#!/bin/sh\n# rebuilt\n").unwrap();
        std::fs::rename(&replacement, &program).unwrap();
        assert_ne!(present, inputs(&sandbox), "the program was replaced");
    }

    /// HUM-508: In `doctor-probe.json` steht nach einem gelungenen
    /// Probestart kein Pfad aus der Umgebung des Daemons: nicht das
    /// Programm, nicht sein Ordner, kein Eintrag des `PATH`, nicht `HOME`.
    /// Schlüssel und Beleg entstehen wie im Doctor (`probe_key`,
    /// `passed_label` und das Urteil über einen bestandenen Start), gelesen
    /// wird die geschriebene Datei.
    ///
    /// Mutationsbeweise: `key: key.id()` statt des Hashes in
    /// `ProbeCache::store` oder der Pfad des Programms zurück in
    /// `passed_label`, und dieser Test wird rot.
    #[test]
    fn the_stored_probe_names_no_path_of_the_daemon() {
        use std::os::unix::process::ExitStatusExt as _;

        use crate::doctor::probe_start::{Ending, ProbeOutcome, passed_label, verdict};
        use crate::launcher::{CheckResult, IsolationCheck};

        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join("home-of-someone");
        let bin = home.join("private-tools-hum508").join("bin");
        let other = dir.path().join("another-path-entry");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let program = bin.join(crate::backend::BWRAP_BACKEND);
        crate::test_support::write_program(&program, "#!/bin/sh\nexit 0\n");
        let path_var = format!("{}:{}", other.display(), bin.display());
        let paths = Paths::new(Env::from_pairs([
            ("HOME", home.to_string_lossy().into_owned()),
            ("PATH", path_var.clone()),
        ]));
        let sandbox = SandboxRef::default();
        let backend = crate::Backend::from(crate::bwrap::BwrapBackend::unchecked(
            &program,
            crate::bwrap::MIN_BWRAP_VERSION,
            paths,
        ));
        let name = crate::backend::BWRAP_BACKEND;
        let key = super::probe_key(name, name, &sandbox, &backend);
        let results: Vec<CheckResult> = IsolationCheck::ALL
            .iter()
            .map(|&check| CheckResult {
                check,
                passed: true,
                evidence: "ok".to_owned(),
                diagnostic: None,
            })
            .collect();
        let ended = Ending::Ended(Ok(std::process::ExitStatus::from_raw(0)));
        let ProbeOutcome::Passed(evidence) =
            verdict(&results, &ended, None, passed_label(&backend))
        else {
            panic!("a passed probe");
        };
        let file = dir.path().join("data").join("doctor-probe.json");
        let cache = ProbeCache::at(&file);
        cache.store(&key, &evidence).unwrap();

        let written = std::fs::read_to_string(&file).unwrap();
        let mut forbidden = vec![
            program.display().to_string(),
            bin.display().to_string(),
            home.display().to_string(),
            dir.path().display().to_string(),
            "private-tools-hum508".to_owned(),
            path_var,
        ];
        forbidden.extend(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|entry| entry.display().to_string())
                .filter(|entry| entry.len() > 1),
        );
        for path in &forbidden {
            assert!(!written.contains(path.as_str()), "{path} in {written}");
        }
        // Der Eintrag gilt weiter: Der Hash des Schlüssels findet ihn.
        if !key.machine.is_empty() {
            assert_eq!(cache.lookup(&key).as_deref(), Some(evidence.as_str()));
        }
    }

    /// HUM-444: Ein gescheiterter Probestart der Kernel-Stufe ist
    /// `DOCTOR_015`, jeder andere bleibt `DOCTOR_014`.
    #[test]
    fn a_failed_probe_of_the_kernel_level_is_its_own_code() {
        let vm = super::probe_failed(crate::VM_BACKEND, "did not start", None);
        assert_eq!(vm.code.as_str(), "DOCTOR_015");
        assert!(
            matches!(&vm.fix, Some(humanitl_core::FixAction::OpenUrl(url)) if url.ends_with("#doctor_015"))
        );
        let other = super::probe_failed("bwrap", "did not start", None);
        assert_eq!(other.code.as_str(), "DOCTOR_014");
    }

    /// HUM-495: Eine Zeile, die eine fehlende Voraussetzung brauchte, ist
    /// ohne Messung (`warn`, `DOCTOR_012`) und nennt die Zeile, auf die sie
    /// wartet; nie `fail`, nie der Befund der anderen Zeile. Mutationsbeweis:
    /// `Unchecked` als `fail` mit `SANDBOX_045`, und der Test wird rot.
    #[test]
    fn an_unchecked_line_is_unmeasured_and_names_the_line_it_needs() {
        let line = super::static_outcome(
            BackendCheckId::VirtioFs,
            super::StaticLine::Unchecked {
                after: BackendCheckId::KernelImage,
                needs: "the kernel image",
            },
        );
        assert_eq!(line.status(), CheckStatus::Warn, "{line:?}");
        assert!(line.is_unmeasured(), "{line:?}");
        assert!(
            line.evidence().contains(
                "it needs the kernel image, and the line kernel reports it missing; \
                 fix the line kernel first"
            ),
            "{line:?}"
        );
        let diagnostic = line.diagnostic().expect("a finding");
        assert_eq!(diagnostic.code.as_str(), "DOCTOR_012");
        assert!(diagnostic.why.contains("virtio_fs"), "{diagnostic:?}");
        assert!(diagnostic.why.contains("the line kernel"), "{diagnostic:?}");
        // Unter einem Backend, das `sandbox.backend` nicht wählt, bleibt sie
        // dieselbe Zeile.
        assert_eq!(line.clone().at_most_warn(), line);
    }

    #[test]
    fn every_backend_check_id_is_snake_case_and_its_own() {
        let ids = [
            BackendCheckId::Admitted,
            BackendCheckId::Available,
            BackendCheckId::Program,
            BackendCheckId::SubordinateIds,
            BackendCheckId::Selinux,
            BackendCheckId::Probe,
            BackendCheckId::Kvm,
            BackendCheckId::Emulator,
            BackendCheckId::FileDaemon,
            BackendCheckId::VsockDevice,
            BackendCheckId::Bubblewrap,
            BackendCheckId::KernelImage,
            BackendCheckId::VirtioFs,
            BackendCheckId::KernelModules,
        ];
        let names: std::collections::BTreeSet<&str> = ids.iter().map(|id| id.as_str()).collect();
        assert_eq!(names.len(), ids.len());
        assert!(
            names
                .iter()
                .all(|name| name.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        );
    }
}
