//! Der Weg einer Stufe, die noch nicht zugelassen ist (HUM-493).
//!
//! Seit HUM-449 steht jede Stufe dieses Builds in [`crate::ADMITTED_BACKENDS`].
//! Die Zulassung bleibt als Gerüst für das nächste Backend (ADR-0020,
//! ADR-0021): Es kommt erst auf die Liste, wenn die Escape-Suite gegen es in
//! der CI grün ist, und bis dahin lehnt jeder Start es mit `SANDBOX_048` ab,
//! während die Suite es über `Backend::detect_unadmitted` startet. Damit
//! dieser Weg nicht ungeprüft bis zum nächsten Backend liegt, nehmen diese
//! Tests je Faden eine Stufe nach der anderen von der Liste
//! ([`crate::test_support::with_admitted`]) und prüfen jede Stelle, die die
//! Zulassung fragt: die Wahl, die Suche eines Starts, den Rückfall der
//! Vorschau, den Knopf in `SANDBOX_036`, die Liste der Backends und die
//! Umgehung der Escape-Suite. Die Stufen kommen aus der Liste selbst, nicht
//! aus Namen im Test; so gilt jeder Test auch für die nächste Stufe, und kein
//! Adaptername steht außerhalb seines Adapters (HUM-145).
//!
//! Mutationsbeweis: `admitted` in `backend.rs`, das immer `Ok(())` liefert,
//! färbt jeden dieser Tests rot; eine Umgehung, die die Zulassung doch fragt,
//! färbt genau den der Umgehung.

#![cfg(test)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use humanitl_config::{Env, MinIsolation, Paths, SandboxBackendName, SandboxRef};
use humanitl_core::{Diagnostic, FixAction};

#[cfg(not(target_os = "macos"))]
use crate::list_backends;
use crate::test_support::with_admitted;
use crate::{ADMITTED_BACKENDS, Backend, backend_name, choose_backend};

/// Eine Stufe dieses Builds: der Name im Schema, der sie wählt, und ihr Name
/// in [`ADMITTED_BACKENDS`].
struct Stage {
    backend: SandboxBackendName,
    name: String,
    /// Hat die Stufe eine Laufzeit, ist sie die der Container-Engine.
    #[cfg_attr(
        target_os = "macos",
        allow(
            dead_code,
            reason = "nur der Listen-Test liest es, unter macOS ohne Engine"
        )
    )]
    has_runtime: bool,
}

/// Die Gruppe `sandbox` mit diesem Backend, ohne Untergrenze.
fn sandbox(backend: SandboxBackendName) -> SandboxRef {
    SandboxRef {
        backend,
        min_isolation: MinIsolation::Policy,
        ..SandboxRef::default()
    }
}

/// Jede Stufe, die ein Name des Schemas außer `auto` wählt, mit der vollen
/// Liste der Zulassung. Jede steht in der Liste: Sonst wäre die Wahl schon
/// hier gescheitert.
fn stages() -> Vec<Stage> {
    let stages: Vec<Stage> = SandboxBackendName::ALL
        .into_iter()
        .filter(|&backend| backend != SandboxBackendName::Auto)
        .map(|backend| {
            let choice = choose_backend(&sandbox(backend), "auto")
                .unwrap_or_else(|err| panic!("{backend:?}: {}: {}", err.code, err.why));
            Stage {
                backend,
                name: choice.stage().into_owned(),
                has_runtime: choice.runtime.is_some(),
            }
        })
        .collect();
    for stage in &stages {
        assert!(ADMITTED_BACKENDS.contains(&stage.name.as_str()));
    }
    assert!(stages.len() >= 2, "nothing left to admit");
    stages
}

/// Die Liste der Zulassung ohne diese Stufe.
fn without(stage: &Stage) -> &'static [&'static str] {
    let rest: Vec<&'static str> = ADMITTED_BACKENDS
        .iter()
        .copied()
        .filter(|&admitted| admitted != stage.name)
        .collect();
    Box::leak(rest.into_boxed_slice())
}

/// Pfade mit `PATH` auf genau `bin` und allen Dateien der Suche in `dir`.
fn paths_with(dir: &Path, bin: &Path) -> Paths {
    std::fs::create_dir_all(bin).expect("bin");
    Paths::new(Env::from_pairs([
        ("HOME", dir.to_string_lossy().into_owned()),
        ("PATH", bin.to_string_lossy().into_owned()),
        (
            "XDG_RUNTIME_DIR",
            dir.join("run").to_string_lossy().into_owned(),
        ),
    ]))
}

fn code<T>(result: &Result<T, Diagnostic>) -> Option<&str> {
    result.as_ref().err().map(|d| d.code.as_str())
}

/// Die Ablehnung einer nicht zugelassenen Stufe: `SANDBOX_048` mit der Stufe
/// im `why` und dem Knopf zurück auf `auto`.
fn assert_not_admitted<T>(result: &Result<T, Diagnostic>, stage: &Stage) {
    let Err(diagnostic) = result else {
        panic!("{} was expected to be refused", stage.name);
    };
    assert_eq!(diagnostic.code.as_str(), "SANDBOX_048", "{diagnostic:?}");
    assert!(diagnostic.why.contains(&stage.name), "{}", diagnostic.why);
    assert!(
        diagnostic.why.contains("Admitted are: "),
        "{}",
        diagnostic.why
    );
    assert_eq!(
        diagnostic.fix,
        Some(FixAction::ChangeSetting {
            key: "sandbox.backend".to_owned(),
            value: crate::AUTO_BACKEND.to_owned(),
        })
    );
}

/// Jeder Weg in einen Start lehnt eine Stufe, die nicht auf der Liste steht,
/// mit `SANDBOX_048` ab, ohne das System zu fragen und ohne auf ein anderes
/// Backend auszuweichen: die Wahl, die Suche und der Rückfall der Vorschau.
/// Jede andere Stufe kommt weiter durch die Wahl.
#[test]
fn an_unadmitted_stage_is_refused_on_every_path_into_a_start() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = paths_with(dir.path(), &dir.path().join("empty-bin"));
    let stages = stages();
    for stage in &stages {
        let chosen = sandbox(stage.backend);
        let name = backend_name(stage.backend);
        with_admitted(without(stage), || {
            assert_not_admitted(&choose_backend(&chosen, "auto"), stage);
            assert_not_admitted(&Backend::detect(name, paths.clone(), &chosen), stage);
            assert_not_admitted(&Backend::fallback(name, paths.clone(), &chosen), stage);
            for other in stages.iter().filter(|other| other.name != stage.name) {
                let kept = choose_backend(&sandbox(other.backend), "auto");
                assert!(kept.is_ok(), "{}: {:?}", other.name, code(&kept));
            }
        });
    }
}

/// Die Umgehung der Escape-Suite fragt die Zulassung nicht und sonst alles
/// wie ein Start: Auf einem `PATH` ohne Programme scheitert sie an der Suche
/// des Backends, nie mit `SANDBOX_048`, während `Backend::detect` mit
/// derselben Liste genau daran scheitert.
#[test]
fn the_bypass_of_the_escape_suite_skips_admission_and_nothing_else() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = paths_with(dir.path(), &dir.path().join("empty-bin"));
    for stage in &stages() {
        let chosen = sandbox(stage.backend);
        let name = backend_name(stage.backend);
        with_admitted(without(stage), || {
            // Die Umgehung erreicht die Suche: Unter Linux scheitert sie am
            // leeren PATH, unter macOS findet sie das Backend über seinen
            // festen Pfad. Den Code der Zulassung liefert sie nie.
            let searched = Backend::detect_unadmitted(name, paths.clone(), &chosen);
            if cfg!(target_os = "macos") {
                assert_ne!(code(&searched), Some("SANDBOX_048"), "{}", stage.name);
            } else {
                let searched = code(&searched).expect("nothing is in PATH");
                assert_ne!(searched, "SANDBOX_048", "{}", stage.name);
            }
            let gated = Backend::detect(name, paths.clone(), &chosen);
            assert_eq!(code(&gated), Some("SANDBOX_048"), "{}", stage.name);
        });
    }
}

/// Verlangt das Sandbox-Profil eine Stufe, die nicht zugelassen ist, bleibt
/// `SANDBOX_036` ohne Knopf: Er führte geradewegs in `SANDBOX_048`. Mit der
/// vollen Liste führt der Knopf zu ihr.
#[test]
fn a_profile_requiring_an_unadmitted_backend_offers_no_button_to_it() {
    let auto = sandbox(SandboxBackendName::Auto);
    let mut checked = 0;
    for stage in &stages() {
        let required = backend_name(stage.backend);
        let Err(admitted) = choose_backend(&auto, required) else {
            // Das Backend des Systems verlangt neben `auto` nichts anderes.
            continue;
        };
        assert_eq!(admitted.code.as_str(), "SANDBOX_036", "{admitted:?}");
        assert!(admitted.fix.is_some(), "{admitted:?}");
        let refused = with_admitted(without(stage), || choose_backend(&auto, required));
        let refused = refused.expect_err("the profile still requires it");
        assert_eq!(refused.code.as_str(), "SANDBOX_036", "{refused:?}");
        assert_eq!(refused.fix, None, "{refused:?}");
        checked += 1;
    }
    assert!(checked > 0, "no stage besides the backend of this system");
}

/// Die Liste der Backends nennt eine Stufe, die nicht zugelassen ist, mit
/// `SANDBOX_048` und nicht verfügbar. Die Container-Engine fragt sie dann gar
/// nicht erst: Eine Attrappe ihres Programms, die auf jede Frage scheitert und
/// jeden Aufruf in eine Datei schreibt, bleibt unberührt, und ihr Eintrag
/// trägt allein `SANDBOX_048`. Geprüft wird jede Stufe, die die Liste auf
/// diesem System führt.
#[cfg(not(target_os = "macos"))]
#[test]
fn the_list_names_an_unadmitted_stage_and_does_not_search_its_engine() {
    let dir = tempfile::tempdir().expect("tempdir");
    let bin = dir.path().join("bin");
    let paths = paths_with(dir.path(), &bin);
    let calls = dir.path().join("calls");
    let codes = |diagnostics: &[Diagnostic]| -> Vec<String> {
        diagnostics
            .iter()
            .map(|d| d.code.as_str().to_owned())
            .collect()
    };
    let stages = stages();
    let engine = stages
        .iter()
        .find(|stage| stage.has_runtime)
        .expect("the container engine is a stage");
    crate::test_support::write_program(
        &bin.join(backend_name(engine.backend)),
        &format!(
            "#!/bin/sh\necho \"$0 $@\" >> '{}'\nexit 99\n",
            calls.display()
        ),
    );
    for stage in &stages {
        let name = backend_name(stage.backend);
        // Die Liste führt nur die Backends dieses Systems.
        if !crate::LISTED_BACKENDS.contains(&name) {
            continue;
        }
        // Die Attrappe zählt nur für diese Stufe: Ist die Engine zugelassen,
        // fragt die Liste sie zu Recht.
        let _ = std::fs::remove_file(&calls);
        let listed = || list_backends(&SandboxRef::default(), "auto", &paths);
        let list = with_admitted(without(stage), listed);
        let entry = list
            .iter()
            .find(|entry| entry.name == name)
            .unwrap_or_else(|| panic!("{name} is not listed: {list:?}"));
        assert!(!entry.available(), "{entry:?}");
        if stage.has_runtime {
            assert_eq!(codes(&entry.diagnostics), ["SANDBOX_048"], "{entry:?}");
            assert_eq!(entry.runtimes.len(), 1, "{entry:?}");
            assert_eq!(codes(&entry.runtimes[0].diagnostics), ["SANDBOX_048"]);
            assert!(
                !calls.exists(),
                "the engine was asked: {}",
                std::fs::read_to_string(&calls).unwrap_or_default()
            );
        } else {
            assert!(
                codes(&entry.diagnostics).contains(&"SANDBOX_048".to_owned()),
                "{entry:?}"
            );
        }
    }
}
