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
//! - `probe`: nur nach einem Probestart, auf Wunsch ([`ProbeMode::Start`])
//!   oder aus dem Zwischenspeicher, solange keine Fassung sich geändert hat
//!   ([`super::ProbeCache`]). `DOCTOR_014`, wenn er scheiterte.
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

use humanitl_config::{Paths, SandboxRef};
use humanitl_core::diagnostics::codes::DOCTOR_014;
use humanitl_core::{Diagnostic, FixAction, Severity};

use super::probe_cache::{ProbeCache, ProbeKey};
use super::probe_start::{ProbeOutcome, probe_start};
use super::{CheckOutcome, DIAGNOSTICS_URL};
use crate::backend::{
    Backend, admitted, backend_name, resolve, runtime_of, stage as stage_name, static_checks,
};
use crate::backend_list::LISTED_BACKENDS;

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
        }
    }
}

impl std::fmt::Display for BackendCheckId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    /// trotzdem da, solange keine Fassung sich geändert hat.
    Cached,
    /// Auf Wunsch: Probestart je zugelassenem, verfügbarem Backend, mit
    /// diesem Shim ([`super::locate_shim`]).
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
/// zugelassenem Backend eine Sandbox.
#[must_use]
pub fn backend_groups(sandbox: &SandboxRef, paths: &Paths, mode: &ProbeMode) -> Vec<BackendGroup> {
    let cache = ProbeCache::of(paths);
    LISTED_BACKENDS
        .iter()
        .map(|&name| group(name, sandbox, paths, mode, &cache))
        .collect()
}

/// Die Gruppe eines Backends.
pub(crate) fn group(
    name: &'static str,
    sandbox: &SandboxRef,
    paths: &Paths,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> BackendGroup {
    let runtime = runtime_of(name, sandbox);
    let stage = stage_name(name, runtime).into_owned();
    let selected = resolve(backend_name(sandbox.backend)).ok() == Some(name);
    let mut checks = admitted(name, runtime).map_or_else(
        |refused| unadmitted_lines(name, &stage, refused, sandbox, paths, mode),
        |()| admitted_lines(name, &stage, sandbox, paths, mode, cache),
    );
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

/// Die Zeilen einer Stufe, die nicht zugelassen ist: `SANDBOX_048`, die
/// Voraussetzungen ohne Start, und mit [`ProbeMode::Start`] eine Zeile
/// `probe`, die sagt, dass nichts startete.
fn unadmitted_lines(
    name: &'static str,
    stage: &str,
    refused: Diagnostic,
    sandbox: &SandboxRef,
    paths: &Paths,
    mode: &ProbeMode,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let mut checks = vec![CheckOutcome::warn(
        BackendCheckId::Admitted,
        format!("{stage} is not admitted yet"),
        refused.clone(),
    )];
    checks.extend(static_checks(name, paths, sandbox).into_iter().map(
        |(id, result)| match result {
            Ok(evidence) => CheckOutcome::ok(id, evidence),
            Err(missing) => CheckOutcome::fail(id, format!("missing: {}", missing.code), missing),
        },
    ));
    if matches!(mode, ProbeMode::Start { .. }) {
        checks.push(CheckOutcome::warn(
            BackendCheckId::Probe,
            format!("not started: {stage} is not admitted yet"),
            refused,
        ));
    }
    checks
}

/// Die Zeilen einer zugelassenen Stufe: die Suche vor jedem Start und der
/// Probestart, gespeichert oder auf Wunsch neu.
fn admitted_lines(
    name: &'static str,
    stage: &str,
    sandbox: &SandboxRef,
    paths: &Paths,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> Vec<CheckOutcome<BackendCheckId>> {
    let mut checks = vec![CheckOutcome::ok(
        BackendCheckId::Admitted,
        format!("{stage} is admitted"),
    )];
    match Backend::detect(name, paths.clone(), sandbox) {
        Ok(backend) => {
            let key = probe_key(stage, &backend);
            checks.push(CheckOutcome::ok(
                BackendCheckId::Available,
                format!("{} {}", backend.program().display(), key.tool_version)
                    .trim_end()
                    .to_owned(),
            ));
            checks.extend(probe_line(&backend, &key, mode, cache));
        }
        Err(missing) => {
            checks.push(CheckOutcome::fail(
                BackendCheckId::Available,
                format!("not usable: {}", missing.code),
                missing.clone(),
            ));
            if matches!(mode, ProbeMode::Start { .. }) {
                checks.push(CheckOutcome::unmeasured(
                    BackendCheckId::Probe,
                    format!("{stage} is not usable here ({})", missing.code),
                    &["humanitl", "sandbox", "backends"],
                ));
            }
        }
    }
    checks
}

/// Die Zeile des Probestarts: aus dem Zwischenspeicher, auf Wunsch aus
/// einem neuen Start, sonst keine.
fn probe_line(
    backend: &Backend,
    key: &ProbeKey,
    mode: &ProbeMode,
    cache: &ProbeCache,
) -> Option<CheckOutcome<BackendCheckId>> {
    if let Some(evidence) = cache.lookup(key) {
        return Some(CheckOutcome::ok(
            BackendCheckId::Probe,
            format!("{evidence} (cached)"),
        ));
    }
    let ProbeMode::Start { shim } = mode else {
        return None;
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

/// `DOCTOR_014`: Der Probestart ist gescheitert. Code und Grund des inneren
/// Befunds stehen im `why`, sein Vorschlag wird übernommen.
fn probe_failed(stage: &str, what: &str, inner: Option<&Diagnostic>) -> Diagnostic {
    let mut why = format!("the probe start of an empty sandbox under {stage} {what}");
    if let Some(inner) = inner {
        let _ = write!(why, "; {}: {}", inner.code, inner.why);
    }
    Diagnostic::builder(DOCTOR_014, Severity::Blocking)
        .why(why)
        .fix(
            inner
                .and_then(|inner| inner.fix.clone())
                .unwrap_or_else(|| FixAction::OpenUrl(format!("{DIAGNOSTICS_URL}#doctor_014"))),
        )
        .build()
}

/// Der Schlüssel des Zwischenspeichers für dieses Backend.
fn probe_key(stage: &str, backend: &Backend) -> ProbeKey {
    ProbeKey {
        stage: stage.to_owned(),
        program: backend.program().display().to_string(),
        tool_version: backend.tool_version(),
        humanitl_version: env!("CARGO_PKG_VERSION").to_owned(),
        machine: super::probe_cache::machine_identity(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{Env, Paths, SandboxBackendName, SandboxRef};

    use super::{BackendCheckId, ProbeMode, group};
    use crate::doctor::{CheckStatus, ProbeCache};

    /// Für eine Stufe vor ihrer Zulassung liefert `doctor --probe`
    /// `SANDBOX_048` und startet keinen Prozess: Jedes Programm im `PATH`
    /// zählt seinen Aufruf in eine Datei, und die Datei entsteht nie. Seit
    /// HUM-464 ist das das VM-Backend der Kernel-Stufe.
    #[test]
    fn before_admission_the_probe_is_refused_and_starts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let calls = dir.path().join("calls");
        for name in ["bwrap", "humanitl-shim"] {
            crate::test_support::write_program(
                &bin.join(name),
                &format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", calls.display()),
            );
        }
        let paths = Paths::new(Env::from_pairs([
            ("HOME", dir.path().to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
        ]));
        let sandbox = SandboxRef {
            backend: SandboxBackendName::Qemu,
            ..SandboxRef::default()
        };
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let mode = ProbeMode::Start {
            shim: bin.join("humanitl-shim"),
        };

        let group = group(crate::VM_BACKEND, &sandbox, &paths, &mode, &cache);

        assert!(group.selected);
        assert_eq!(group.stage, crate::VM_BACKEND);
        for id in [BackendCheckId::Admitted, BackendCheckId::Probe] {
            let line = group.get(id).expect("the line is there");
            assert_eq!(line.status(), CheckStatus::Warn, "{line:?}");
            assert_eq!(
                line.diagnostic().map(|d| d.code.as_str()),
                Some("SANDBOX_048")
            );
        }
        assert!(group.get(BackendCheckId::Available).is_none());
        assert!(
            !calls.exists(),
            "a process was started: {:?}",
            std::fs::read_to_string(&calls)
        );
        assert!(!cache.path().exists(), "nothing was probed, nothing cached");
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
