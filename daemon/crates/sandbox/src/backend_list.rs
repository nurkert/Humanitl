//! Die Backends dieses Systems als Liste (HUM-432), für `SandboxBackends`.
//!
//! Oberfläche und Kommandozeile prüfen nichts selbst (ADR-018): Welche
//! Backends es gibt, welche Stufe sie bieten, ob ein Start damit ginge und was
//! fehlt, steht hier, an derselben Stelle wie die Wahl ([`crate::backend`]).
//! Jeder Eintrag geht dieselben Prüfungen wie [`crate::choose_backend`] und
//! [`crate::Backend::detect`] durch, nur ohne beim ersten Befund aufzuhören:
//! Die Liste nennt alles, was einem Start im Weg steht.
//!
//! Die Liste startet keine Sandbox. Sie fragt jedes Backend wie die Suche vor
//! jedem Start ([`crate::Backend::detect`], ohne die Zulassung): unter bwrap
//! `bwrap --version` und den Nutzer-Namensraum, unter der Container-Engine
//! seit ihrer Zulassung (HUM-448) Programm, Fassung, `info`, untergeordnete
//! IDs und `SELinux`, je mit der Frist der Suche; vor der Zulassung einer
//! Stufe nur, ob das Programm da ist. Unter dem VM-Backend der Kernel-Stufe
//! nur seine Voraussetzungen (HUM-464): Gerät, Programme und ihre Hilfe,
//! Kernel und Module. Den Probestart macht `doctor` (HUM-436).
//!
//! `available` heißt nie, dass ein anderes Backend einspringt: Ein nicht
//! verfügbares gewähltes Backend lässt den Start scheitern.

use humanitl_config::{Paths, PodmanRuntime, SandboxRef};
use humanitl_core::diagnostics::codes::SANDBOX_034;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::backend::{
    BWRAP_BACKEND, BackendChoice, PODMAN_BACKEND, SEATBELT_BACKEND, VM_BACKEND, admitted,
    backend_name, below_floor, detect_in_build, isolation_of, kernel_level_missing,
    profile_requires_another, required_by_profile, resolve,
};
use crate::launcher::IsolationLevel;

/// Die Backends, die es auf dieser Art System geben kann, in der Reihenfolge
/// der Liste: unter macOS Seatbelt, sonst bwrap und Podman, unter Linux auf
/// x86-64 dazu das VM-Backend der Kernel-Stufe (HUM-464). Podman unter macOS
/// liefe in einer Podman-Maschine, die `SANDBOX_038` ablehnt (HUM-433); es
/// steht dort deshalb gar nicht erst zur Wahl.
pub const LISTED_BACKENDS: &[&str] = if cfg!(target_os = "macos") {
    &[SEATBELT_BACKEND]
} else if cfg!(target_arch = "x86_64") {
    &[BWRAP_BACKEND, PODMAN_BACKEND, VM_BACKEND]
} else {
    &[BWRAP_BACKEND, PODMAN_BACKEND]
};

/// Die Laufzeiten unter Podman, in der Reihenfolge der Liste. Seit HUM-464
/// nur noch die des geteilten Kerns; die Kernel-Stufe ist ein eigenes Backend.
const PODMAN_RUNTIMES: [PodmanRuntime; 1] = [PodmanRuntime::Crun];

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
    /// Die Laufzeiten, unter Podman `crun`; sonst leer.
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
    /// `crun`.
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

/// Was dem System für dieses Backend fehlt: der Befund der Suche, die jeder
/// Start macht ([`detect_in_build`]).
///
/// Unter dem VM-Backend der Kernel-Stufe nur die Prüfung seiner
/// Voraussetzungen (`SANDBOX_045`): Einen Adapter gibt es bis HUM-444 nicht,
/// und dass die Stufe nicht zugelassen ist, sagt `SANDBOX_048` daneben.
///
/// Unter der Container-Engine zuerst, ob es das Programm gibt, mit dem Weg
/// zurück im Text. Danach, wenn ihre gemeinsame Stufe zugelassen ist, die
/// Suche mit dieser Stufe (HUM-448): Die Liste nennt ein Backend nie
/// verfügbar, dessen Start schon an der Suche scheiterte. Gefragt wird die
/// gemeinsame Stufe, gleich welche Laufzeit eingestellt ist: Was das System
/// der Engine schuldig bleibt, fehlt jeder Laufzeit; was einer Stufe fehlt,
/// sagen Untergrenze und Zulassung je Laufzeit.
fn system_missing(name: &'static str, sandbox: &SandboxRef, paths: &Paths) -> Option<Diagnostic> {
    if name == VM_BACKEND {
        return kernel_level_missing(paths, sandbox);
    }
    if name != PODMAN_BACKEND {
        return detect_in_build(name, paths.clone(), sandbox).err();
    }
    if let Some(missing) = podman_missing(sandbox, paths) {
        return Some(missing);
    }
    let shared = PodmanRuntime::Crun;
    if admitted(name, Some(shared)).is_err() {
        return None;
    }
    let mut searched = sandbox.clone();
    searched.podman.runtime = shared;
    detect_in_build(name, paths.clone(), &searched).err()
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

    /// Pfade mit `PATH` auf genau diesen Ordner und dem Laufzeitverzeichnis
    /// in `dir`: Die Suche der Engine legt dort ihre Dateien an, nie im
    /// Laufzeitverzeichnis des Menschen am Rechner.
    fn paths_with(dir: &Path, bin: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.to_string_lossy().into_owned()),
            ("PATH", bin.to_string_lossy().into_owned()),
            (
                "XDG_RUNTIME_DIR",
                dir.join("run").to_string_lossy().into_owned(),
            ),
        ]))
    }

    /// Eine Attrappe der Engine in `bin`, die jede Frage mit Exit 99
    /// beantwortet: Sie ist da, und ihre Suche scheitert.
    fn fake_podman(bin: &Path) {
        fake_engine(bin, "exit 99\n");
    }

    /// Eine Attrappe der Engine in `bin` mit diesem Rumpf eines Skripts.
    fn fake_engine(bin: &Path, body: &str) {
        std::fs::create_dir_all(bin).expect("bin");
        crate::test_support::write_program(
            &bin.join(PODMAN_BACKEND),
            &format!("#!/bin/sh\n{body}"),
        );
    }

    /// Der Befund, mit dem die Suche eines Starts die gemeinsame Stufe auf
    /// diesen Pfaden ablehnt, oder keiner.
    fn search_of_a_start(paths: &Paths) -> Option<Diagnostic> {
        crate::Backend::detect(PODMAN_BACKEND, paths.clone(), &SandboxRef::default()).err()
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<&str> {
        diagnostics.iter().map(|d| d.code.as_str()).collect()
    }

    fn entry<'a>(list: &'a [BackendEntry], name: &str) -> &'a BackendEntry {
        list.iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} is not listed: {list:?}"))
    }

    /// Seit HUM-448 fragt die Liste die Engine wie die Suche jedes Starts:
    /// Eine Attrappe, die da ist und auf jede Frage scheitert, ist nicht
    /// verfügbar, mit dem Befund der Suche samt `why` und `fix`; ihre eine
    /// Laufzeit ebenso.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn an_engine_whose_search_fails_is_listed_with_the_finding_of_the_search() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("bin");
        fake_podman(&bin);
        let paths = paths_with(dir.path(), &bin);
        let searched = search_of_a_start(&paths).expect("the fake fails every question");
        let list = list_backends(&SandboxRef::default(), "auto", &paths);
        let engine = entry(&list, PODMAN_BACKEND);
        assert!(!engine.available() && !engine.selected, "{engine:?}");
        assert_eq!(engine.isolation, IsolationLevel::Namespaces);
        assert_eq!(engine.diagnostics, std::slice::from_ref(&searched));
        assert!(
            !searched.why.is_empty() && searched.fix.is_some(),
            "{searched:?}"
        );
        assert_ne!(searched.code.as_str(), "SANDBOX_048");
        let shared = searched.code.as_str();
        assert_eq!(engine.runtimes.len(), 1, "{engine:?}");
        assert_eq!(codes(&engine.runtimes[0].diagnostics), [shared]);
    }

    /// Verfügbar heißt: Die Suche eines Starts besteht. Eine Attrappe, die
    /// Fassung und `info` richtig beantwortet, ist genau dann verfügbar, wenn
    /// die Suche auf diesem Rechner mit ihr besteht (untergeordnete IDs und
    /// `SELinux` sind die des Rechners), und sonst steht derselbe Befund da.
    /// Gestartet wird dabei nichts: Die Attrappe kennt nur diese zwei Fragen.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_list_says_available_exactly_when_the_search_of_a_start_does() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("bin");
        fake_engine(
            &bin,
            "case \" $* \" in\n\
             *' version '*) echo 5.4.0 ;;\n\
             *' info '*) echo 'true|false|crun|v2|[cpu memory pids]' ;;\n\
             *) exit 99 ;;\n\
             esac\n",
        );
        let paths = paths_with(dir.path(), &bin);
        let searched = search_of_a_start(&paths);
        let list = list_backends(&SandboxRef::default(), "auto", &paths);
        let engine = entry(&list, PODMAN_BACKEND);
        assert_eq!(engine.diagnostics, Vec::from_iter(searched.clone()));
        assert_eq!(engine.available(), searched.is_none(), "{engine:?}");
        assert_eq!(engine.runtimes[0].available(), searched.is_none());
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
        assert_eq!(codes(&podman.diagnostics), ["SANDBOX_034"]);
        let missing = &podman.diagnostics[0];
        assert!(missing.fix.is_some(), "{missing:?}");
        assert!(missing.why.contains("--backend bwrap"), "{}", missing.why);
        assert_eq!(codes(&podman.runtimes[0].diagnostics), ["SANDBOX_034"]);
        // Ein ausdrücklich genanntes Programm, das es nicht gibt, ebenso.
        let mut sandbox = SandboxRef::default();
        sandbox.podman.program = Some(dir.path().join("nope").join("podman"));
        fake_podman(&dir.path().join("bin"));
        let list = list_backends(
            &sandbox,
            "auto",
            &paths_with(dir.path(), &dir.path().join("bin")),
        );
        assert_eq!(codes(&entry(&list, "podman").diagnostics), ["SANDBOX_034"]);
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
            LISTED_BACKENDS
        );
        assert_eq!(LISTED_BACKENDS[..2], ["bwrap", "podman"]);
        let bwrap = entry(&list, "bwrap");
        assert!(bwrap.selected);
        assert_eq!(bwrap.isolation, IsolationLevel::Namespaces);
        assert_eq!(codes(&bwrap.diagnostics), ["SANDBOX_001"]);
        assert!(bwrap.runtimes.is_empty());
        assert_eq!(list.iter().filter(|e| e.selected).count(), 1);
    }

    /// Die Untergrenze `kernel` macht bwrap und Podman unverfügbar
    /// (`SANDBOX_035`); das VM-Backend liegt nicht darunter und scheitert nur
    /// an der Zulassung.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_floor_above_a_backend_is_named_in_its_entry() {
        let sandbox = SandboxRef {
            backend: SandboxBackendName::Podman,
            min_isolation: MinIsolation::Kernel,
            ..SandboxRef::default()
        };
        let list = list_backends_with(&sandbox, "auto", |_| None);
        let bwrap = entry(&list, "bwrap");
        assert!(!bwrap.selected);
        assert!(
            codes(&bwrap.diagnostics).contains(&"SANDBOX_035"),
            "{bwrap:?}"
        );
        let podman = entry(&list, "podman");
        assert!(podman.selected);
        assert_eq!(codes(&podman.diagnostics), ["SANDBOX_035"]);
        if cfg!(target_arch = "x86_64") {
            let kernel = entry(&list, VM_BACKEND);
            assert_eq!(
                codes(&kernel.diagnostics),
                ["SANDBOX_048"],
                "the VM backend is not below kernel"
            );
        }
    }

    /// Das VM-Backend der Kernel-Stufe steht in der Liste (HUM-464): Stufe
    /// `kernel`, nicht verfügbar, mit `SANDBOX_048` bis HUM-449, ohne
    /// Laufzeiten. Fehlt dem Rechner etwas, steht `SANDBOX_045` mit dem
    /// ersten Fehlenden davor; hier fehlt jedes Programm im leeren `PATH`.
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn the_kernel_level_is_listed_unavailable_until_its_admission() {
        let sandbox = SandboxRef {
            backend: SandboxBackendName::Qemu,
            ..SandboxRef::default()
        };
        let installed = list_backends_with(&sandbox, "auto", |_| None);
        let kernel = entry(&installed, VM_BACKEND);
        assert!(kernel.selected && !kernel.available(), "{kernel:?}");
        assert_eq!(kernel.isolation, IsolationLevel::Kernel);
        assert_eq!(codes(&kernel.diagnostics), ["SANDBOX_048"]);
        assert!(kernel.runtimes.is_empty());

        let dir = tempfile::tempdir().expect("tempdir");
        let bin = dir.path().join("empty-bin");
        std::fs::create_dir_all(&bin).expect("bin");
        let list = list_backends(&sandbox, "auto", &paths_with(dir.path(), &bin));
        let kernel = entry(&list, VM_BACKEND);
        assert_eq!(
            codes(&kernel.diagnostics),
            ["SANDBOX_045", "SANDBOX_048"],
            "{kernel:?}"
        );
        let missing = &kernel.diagnostics[0];
        assert!(missing.fix.is_some(), "{missing:?}");
        assert!(missing.why.contains("--backend bwrap"), "{}", missing.why);
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
