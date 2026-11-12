//! Die Backends dieses Systems als Liste (HUM-432), für `SandboxBackends`.
//!
//! Oberfläche und Kommandozeile prüfen nichts selbst (ADR-018): Welche
//! Backends es gibt, welche Stufe sie bieten, ob ein Start damit ginge und was
//! fehlt, steht hier, an derselben Stelle wie die Wahl ([`crate::backend`]).
//! Jeder Eintrag geht dieselben Prüfungen wie [`crate::choose_backend`] und
//! [`crate::Backend::detect`] durch, nur ohne beim ersten Befund aufzuhören:
//! Die Liste nennt alles, was einem Start im Weg steht.
//!
//! Die Liste startet nichts. Unter bwrap fragt sie das Programm wie die Suche
//! vor jedem Start (`bwrap --version`, Nutzer-Namensraum); unter Podman nur,
//! ob das Programm da ist. Den Probestart macht `doctor` (HUM-436).
//!
//! `available` heißt nie, dass ein anderes Backend einspringt: Ein nicht
//! verfügbares gewähltes Backend lässt den Start scheitern.

use humanitl_config::{Paths, PodmanRuntime, SandboxRef};
use humanitl_core::diagnostics::codes::SANDBOX_034;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::backend::{
    BWRAP_BACKEND, BackendChoice, PODMAN_BACKEND, SEATBELT_BACKEND, admitted, backend_name,
    below_floor, detect_in_build, isolation_of, profile_requires_another, required_by_profile,
    resolve,
};
use crate::launcher::IsolationLevel;

/// Die Backends, die es auf dieser Art System geben kann, in der Reihenfolge
/// der Liste: unter macOS Seatbelt, sonst bwrap und Podman. Podman unter macOS
/// liefe in einer Podman-Maschine, die `SANDBOX_038` ablehnt (HUM-433); es
/// steht dort deshalb gar nicht erst zur Wahl.
pub const LISTED_BACKENDS: &[&str] = if cfg!(target_os = "macos") {
    &[SEATBELT_BACKEND]
} else {
    &[BWRAP_BACKEND, PODMAN_BACKEND]
};

/// Die Laufzeiten unter Podman, in der Reihenfolge der Liste.
const PODMAN_RUNTIMES: [PodmanRuntime; 2] = [PodmanRuntime::Crun, PodmanRuntime::Krun];

/// Der Suchpfad, wenn die Umgebung keinen hat; derselbe wie bei bwrap.
const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Ein Backend der Liste und was ihm fehlt.
#[derive(Debug, Clone, PartialEq)]
pub struct BackendEntry {
    /// Der Name in `sandbox.backend`.
    pub name: &'static str,
    /// Die Stufe; unter Podman die der gewählten Laufzeit.
    pub isolation: IsolationLevel,
    /// Ob `sandbox.backend` dieses Backend wählt (`auto` aufgelöst).
    pub selected: bool,
    /// Was einem Start im Weg steht; leer, wenn er ginge.
    pub diagnostics: Vec<Diagnostic>,
    /// Die Laufzeiten, unter Podman `crun` und `krun`; sonst leer.
    pub runtimes: Vec<RuntimeEntry>,
}

impl BackendEntry {
    /// Ob ein Start mit diesem Backend durchginge.
    #[must_use]
    pub const fn available(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Eine Laufzeit unter Podman und was ihr fehlt.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeEntry {
    /// `crun` oder `krun`.
    pub runtime: PodmanRuntime,
    /// Die Stufe mit dieser Laufzeit.
    pub isolation: IsolationLevel,
    /// Ob `sandbox.podman.runtime` diese Laufzeit wählt.
    pub selected: bool,
    /// Was einem Start mit dieser Laufzeit im Weg steht.
    pub diagnostics: Vec<Diagnostic>,
}

impl RuntimeEntry {
    /// Ob ein Start mit dieser Laufzeit durchginge.
    #[must_use]
    pub const fn available(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Die Backends dieses Systems mit ihrer Stufe, ihrer Verfügbarkeit und
/// dem, was fehlt (HUM-432).
///
/// `sandbox` ist die aufgelöste Gruppe `sandbox` ohne Projekt: Ihr Backend
/// ist das gewählte, ihre Untergrenze die, an der jeder Eintrag gemessen
/// wird. `profile_backend` ist das Feld `[sandbox] backend` des
/// Sandbox-Profils aus `sandbox.profile`. Die Befunde je Eintrag, in dieser
/// Reihenfolge: was dem System fehlt (`SANDBOX_034`, unter bwrap
/// `SANDBOX_001` bis `SANDBOX_003`), die Anforderung des Profils
/// (`SANDBOX_036`), die Untergrenze (`SANDBOX_035`) und die Zulassung
/// (`SANDBOX_048`). Das System eines nicht zugelassenen Backends wird
/// trotzdem gefragt, damit die Liste sagt, was nach der Zulassung noch
/// fehlte. Ein Eintrag ohne Befund sagt voraus, dass ein Start ohne
/// Projekt-Profil mit diesem Backend die Wahl besteht.
#[must_use]
pub fn list_backends(
    sandbox: &SandboxRef,
    profile_backend: &str,
    paths: &Paths,
) -> Vec<BackendEntry> {
    list_backends_with(sandbox, profile_backend, |name| {
        system_missing(name, sandbox, paths)
    })
}

/// Wie [`list_backends`], mit einer anderen Frage an das System: `missing`
/// nennt je Backend, was ihm auf dem System fehlt, oder nichts. Der Fake des
/// Daemons spielt damit ein System, auf dem alles installiert ist, und
/// bekommt Untergrenze, Zulassung und Laufzeiten aus derselben Liste wie der
/// echte Daemon (HUM-432).
#[must_use]
pub fn list_backends_with(
    sandbox: &SandboxRef,
    profile_backend: &str,
    missing: impl Fn(&'static str) -> Option<Diagnostic>,
) -> Vec<BackendEntry> {
    let chosen = resolve(backend_name(sandbox.backend)).ok();
    let floor = IsolationLevel::from(sandbox.min_isolation);
    let required = required_by_profile(profile_backend);
    LISTED_BACKENDS
        .iter()
        .map(|&name| {
            let selected = chosen == Some(name);
            let missing: Vec<Diagnostic> = missing(name)
                .into_iter()
                .chain(profile_refusal(&required, name))
                .collect();
            if name == PODMAN_BACKEND {
                podman_entry(sandbox, &missing, floor, selected)
            } else {
                let diagnostics = missing
                    .into_iter()
                    .chain(stage_refusals(name, None, floor))
                    .collect();
                BackendEntry {
                    name,
                    isolation: isolation_of(name, None),
                    selected,
                    diagnostics,
                    runtimes: Vec::new(),
                }
            }
        })
        .collect()
}

/// Was dem System für dieses Backend fehlt: unter Podman das Programm,
/// sonst der Befund der Suche des Backends ([`detect_in_build`]).
fn system_missing(name: &'static str, sandbox: &SandboxRef, paths: &Paths) -> Option<Diagnostic> {
    if name == PODMAN_BACKEND {
        podman_missing(sandbox, paths)
    } else {
        detect_in_build(name, paths.clone(), sandbox).err()
    }
}

/// `SANDBOX_036`, wenn das Sandbox-Profil ein anderes Backend verlangt, oder
/// der Befund eines Namens im Profil, den dieses Schema nicht kennt.
fn profile_refusal(
    required: &Result<Option<&'static str>, Diagnostic>,
    name: &'static str,
) -> Option<Diagnostic> {
    match required {
        Ok(Some(required)) if *required != name => Some(profile_requires_another(required, name)),
        Ok(_) => None,
        Err(unknown) => Some(unknown.clone()),
    }
}

/// Der Eintrag für Podman: Das Programm fehlt oder nicht, dazu je Laufzeit
/// Untergrenze und Zulassung. Das Backend selbst trägt die Befunde der
/// gewählten Laufzeit.
fn podman_entry(
    sandbox: &SandboxRef,
    missing: &[Diagnostic],
    floor: IsolationLevel,
    selected: bool,
) -> BackendEntry {
    let runtimes: Vec<RuntimeEntry> = PODMAN_RUNTIMES
        .into_iter()
        .map(|runtime| {
            let diagnostics = missing
                .iter()
                .cloned()
                .chain(stage_refusals(PODMAN_BACKEND, Some(runtime), floor))
                .collect();
            RuntimeEntry {
                runtime,
                isolation: isolation_of(PODMAN_BACKEND, Some(runtime)),
                selected: runtime == sandbox.podman.runtime,
                diagnostics,
            }
        })
        .collect();
    let chosen = runtimes
        .iter()
        .find(|entry| entry.selected)
        .map_or_else(Vec::new, |entry| entry.diagnostics.clone());
    BackendEntry {
        name: PODMAN_BACKEND,
        isolation: isolation_of(PODMAN_BACKEND, Some(sandbox.podman.runtime)),
        selected,
        diagnostics: chosen,
        runtimes,
    }
}

/// Untergrenze und Zulassung einer Stufe, wie [`crate::choose_backend`] sie
/// prüft, aber beide statt nur der ersten.
fn stage_refusals(
    name: &'static str,
    runtime: Option<PodmanRuntime>,
    floor: IsolationLevel,
) -> Vec<Diagnostic> {
    let choice = BackendChoice {
        name,
        runtime,
        isolation: isolation_of(name, runtime),
    };
    let mut refusals = Vec::new();
    if choice.isolation < floor {
        refusals.push(below_floor(&choice, floor));
    }
    if let Err(refused) = admitted(name, runtime) {
        refusals.push(refused);
    }
    refusals
}

/// `SANDBOX_034`, wenn es das Programm von Podman nicht gibt: das aus
/// `sandbox.podman.program`, sonst das erste im `PATH` des Daemons. Mehr
/// fragt die Liste nicht; Fassung, rootless und Laufzeit prüft der Adapter
/// (HUM-433).
fn podman_missing(sandbox: &SandboxRef, paths: &Paths) -> Option<Diagnostic> {
    let (found, looked) = if let Some(program) = sandbox.podman.program.as_deref() {
        (
            crate::agent::is_executable_file(program),
            format!(
                "sandbox.podman.program = {:?}",
                program.display().to_string()
            ),
        )
    } else {
        let path = paths.env().non_empty("PATH").unwrap_or(DEFAULT_PATH);
        let found = std::env::split_paths(path)
            .filter(|dir| !dir.as_os_str().is_empty())
            .any(|dir| crate::agent::is_executable_file(&dir.join(PODMAN_BACKEND)));
        (found, format!("PATH={path}"))
    };
    if found {
        return None;
    }
    Some(
        Diagnostic::builder(SANDBOX_034, Severity::Blocking)
            .why(format!(
                "sandbox.backend = \"{PODMAN_BACKEND}\" needs the program {PODMAN_BACKEND}, and \
                 there is no executable one at {looked}; the start does not switch to another \
                 backend on its own. Install Podman, or start with --backend {BWRAP_BACKEND}"
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.backend".to_owned(),
                value: crate::AUTO_BACKEND.to_owned(),
            })
            .build(),
    )
}

// Jeder Fall unten fragt Linux-Backends ab; unter macOS bliebe das Modul
// leer und seine Helfer toter Code.
#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use humanitl_config::{Env, MinIsolation, SandboxBackendName};

    use super::*;

    /// Pfade mit `PATH` auf genau diesen Ordner.
    fn paths_with(dir: &Path, bin: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
        ]))
    }

    /// Ein ausführbares `podman` in `bin`, das nie läuft: Die Liste fragt nur,
    /// ob es da ist.
    fn fake_podman(bin: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(bin).expect("bin");
        let program = bin.join("podman");
        std::fs::write(&program, "#!/bin/sh\nexit 99\n").expect("podman");
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("mode");
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    fn entry<'a>(list: &'a [BackendEntry], name: &str) -> &'a BackendEntry {
        list.iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} is not listed: {list:?}"))
    }

    /// Vor HUM-448 ist Podman nicht verfügbar, auch wenn es installiert ist:
    /// `SANDBOX_048`, und kein `SANDBOX_034`. Beide Laufzeiten ebenso.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn an_installed_podman_is_listed_but_not_available_until_it_is_admitted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("bin");
        fake_podman(&bin);
        let list = list_backends(
            &SandboxRef::default(),
            "auto",
            &paths_with(dir.path(), &bin),
        );
        let podman = entry(&list, "podman");
        assert!(!podman.available(), "{podman:?}");
        assert!(!podman.selected);
        assert_eq!(codes(&podman.diagnostics), ["SANDBOX_048"]);
        assert_eq!(podman.isolation, IsolationLevel::Namespaces);
        let runtimes: Vec<_> = podman
            .runtimes
            .iter()
            .map(|r| (r.runtime, r.isolation, r.selected, r.available()))
            .collect();
        assert_eq!(
            runtimes,
            [
                (PodmanRuntime::Crun, IsolationLevel::Namespaces, true, false),
                (PodmanRuntime::Krun, IsolationLevel::Kernel, false, false),
            ]
        );
        for runtime in &podman.runtimes {
            assert_eq!(codes(&runtime.diagnostics), ["SANDBOX_048"], "{runtime:?}");
        }
    }

    /// Ohne `podman` im `PATH` steht zusätzlich `SANDBOX_034` mit `fix`, und
    /// der Weg zurück steht im Text.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_missing_podman_adds_sandbox_034_with_a_fix() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("empty-bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let list = list_backends(
            &SandboxRef::default(),
            "auto",
            &paths_with(dir.path(), &bin),
        );
        let podman = entry(&list, "podman");
        assert_eq!(codes(&podman.diagnostics), ["SANDBOX_034", "SANDBOX_048"]);
        let missing = &podman.diagnostics[0];
        assert!(missing.fix.is_some(), "{missing:?}");
        assert!(missing.why.contains("--backend bwrap"), "{}", missing.why);
        for runtime in &podman.runtimes {
            assert_eq!(
                codes(&runtime.diagnostics),
                ["SANDBOX_034", "SANDBOX_048"],
                "{runtime:?}"
            );
        }
        // Ein ausdrücklich genanntes Programm, das es nicht gibt, ebenso.
        let mut sandbox = SandboxRef::default();
        sandbox.podman.program = Some(dir.path().join("nope").join("podman"));
        fake_podman(&dir.path().join("bin"));
        let list = list_backends(
            &sandbox,
            "auto",
            &paths_with(dir.path(), &dir.path().join("bin")),
        );
        assert_eq!(
            codes(&entry(&list, "podman").diagnostics),
            ["SANDBOX_034", "SANDBOX_048"]
        );
    }

    /// bwrap steht da wie vor HUM-432: gewählt unter `auto`, mit dem Befund
    /// seiner Suche, wenn es fehlt, und sonst nichts.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn bwrap_is_selected_by_auto_and_names_what_its_search_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("empty-bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let list = list_backends(
            &SandboxRef::default(),
            "auto",
            &paths_with(dir.path(), &bin),
        );
        assert_eq!(
            list.iter().map(|e| e.name).collect::<Vec<_>>(),
            ["bwrap", "podman"]
        );
        let bwrap = entry(&list, "bwrap");
        assert!(bwrap.selected);
        assert_eq!(bwrap.isolation, IsolationLevel::Namespaces);
        assert_eq!(codes(&bwrap.diagnostics), ["SANDBOX_001"]);
        assert!(bwrap.runtimes.is_empty());
        assert_eq!(list.iter().filter(|e| e.selected).count(), 1);
    }

    /// Die Untergrenze `kernel` macht bwrap und Podman mit `crun`
    /// unverfügbar (`SANDBOX_035`); `krun` liegt nicht darunter und scheitert
    /// nur an der Zulassung.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_floor_above_a_backend_is_named_in_its_entry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("bin");
        fake_podman(&bin);
        let sandbox = SandboxRef {
            backend: SandboxBackendName::Podman,
            min_isolation: MinIsolation::Kernel,
            ..SandboxRef::default()
        };
        let list = list_backends(&sandbox, "auto", &paths_with(dir.path(), &bin));
        let bwrap = entry(&list, "bwrap");
        assert!(!bwrap.selected);
        assert!(
            codes(&bwrap.diagnostics).contains(&"SANDBOX_035"),
            "{bwrap:?}"
        );
        let podman = entry(&list, "podman");
        assert!(podman.selected);
        assert_eq!(codes(&podman.diagnostics), ["SANDBOX_035", "SANDBOX_048"]);
        assert_eq!(
            codes(&podman.runtimes[1].diagnostics),
            ["SANDBOX_048"],
            "krun is not below kernel"
        );
    }

    /// Ein Sandbox-Profil, das Podman verlangt, macht das gewählte Backend
    /// des Systems unverfügbar (`SANDBOX_036`), wie der Start es ablehnte;
    /// Podman selbst trägt den Befund nicht.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_profile_requiring_another_backend_is_named_in_the_entry() {
        let list = list_backends_with(&SandboxRef::default(), "podman", |_| None);
        let system = entry(&list, crate::PLATFORM_BACKEND_NAME);
        assert!(system.selected && !system.available(), "{system:?}");
        assert_eq!(codes(&system.diagnostics), ["SANDBOX_036"]);
        let podman = entry(&list, "podman");
        assert!(
            !codes(&podman.diagnostics).contains(&"SANDBOX_036"),
            "{podman:?}"
        );
        for runtime in &podman.runtimes {
            assert!(!codes(&runtime.diagnostics).contains(&"SANDBOX_036"));
        }
        let auto = list_backends_with(&SandboxRef::default(), "auto", |_| None);
        assert!(entry(&auto, crate::PLATFORM_BACKEND_NAME).available());
    }
}
