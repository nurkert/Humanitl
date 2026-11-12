//! Der Doctor je Backend für Podman, vor der Zulassung (HUM-436).
//!
//! Die Tests stehen im Adapter, weil sie Podman beim Namen nennen müssen;
//! außerhalb des Adapters spricht der Code nur vom Backend
//! (`tools/check_coupling.py`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_config::{Env, Paths, PodmanConfig, SandboxBackendName, SandboxRef};

use crate::doctor::backends::group;
use crate::doctor::{BackendCheckId, CheckStatus, ProbeCache, ProbeMode};

/// Die Gruppe `sandbox` mit gewähltem Podman und diesem Programm.
fn podman_with(program: std::path::PathBuf) -> SandboxRef {
    SandboxRef {
        backend: SandboxBackendName::Podman,
        podman: PodmanConfig {
            program: Some(program),
            ..PodmanConfig::default()
        },
        ..SandboxRef::default()
    }
}

/// Vor HUM-448 liefert `doctor --probe` für Podman `SANDBOX_048` und
/// startet keinen Prozess: Die Attrappe für `podman` zählt jeden Aufruf
/// in eine Datei, und die Datei entsteht nie.
#[test]
fn before_admission_the_probe_of_podman_is_refused_and_starts_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let calls = dir.path().join("calls");
    let podman = bin.join("podman");
    crate::test_support::write_program(
        &podman,
        &format!("#!/bin/sh\necho \"$@\" >> '{}'\nexit 0\n", calls.display()),
    );
    let shim = bin.join("humanitl-shim");
    crate::test_support::write_program(
        &shim,
        &format!("#!/bin/sh\necho shim >> '{}'\nexit 0\n", calls.display()),
    );
    // Ein Laufzeitverzeichnis, in dem der Adapter seines anlegen könnte:
    // Ohne es scheiterte ein Start von Podman schon davor, und der Test
    // bewiese nicht, dass der Doctor Podman nicht fragt.
    let runtime = dir.path().join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    let paths = Paths::new(Env::from_pairs([
        ("HOME", dir.path().to_string_lossy().into_owned()),
        ("PATH", bin.to_string_lossy().into_owned()),
        ("XDG_RUNTIME_DIR", runtime.to_string_lossy().into_owned()),
    ]));
    let sandbox = podman_with(podman.clone());
    let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));

    let group = group(
        "podman",
        &sandbox,
        &paths,
        &ProbeMode::Start { shim },
        &cache,
    );

    assert!(group.selected);
    assert_eq!(group.stage, "podman/crun");
    let admitted = group.get(BackendCheckId::Admitted).expect("admitted");
    assert_eq!(admitted.status(), CheckStatus::Warn);
    assert_eq!(
        admitted.diagnostic().map(|d| d.code.as_str()),
        Some("SANDBOX_048")
    );
    let probe = group.get(BackendCheckId::Probe).expect("probe");
    assert_eq!(probe.status(), CheckStatus::Warn);
    assert_eq!(
        probe.diagnostic().map(|d| d.code.as_str()),
        Some("SANDBOX_048")
    );
    assert!(probe.diagnostic().unwrap().fix.is_some());
    // Die statischen Zeilen stehen da: Das Programm ist gefunden, ohne
    // dass es lief.
    let program = group.get(BackendCheckId::Program).expect("program");
    assert_eq!(program.status(), CheckStatus::Ok, "{program:?}");
    assert!(
        !group
            .checks
            .iter()
            .any(|c| c.id() == BackendCheckId::Available)
    );
    assert!(
        !calls.exists(),
        "a process was started: {:?}",
        std::fs::read_to_string(&calls)
    );
    assert!(!cache.path().exists(), "nothing was probed, nothing cached");
}

/// Ein falsches `sandbox.podman.program` ist in der Gruppe des gewählten
/// Backends `fail` mit `SANDBOX_034` und dem Installationsbefehl; ist
/// Podman nicht gewählt, bleibt dieselbe Zeile `warn`.
#[test]
fn a_wrong_podman_program_fails_only_where_podman_is_chosen() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(Env::from_pairs([
        ("HOME", dir.path().to_string_lossy().into_owned()),
        (
            "PATH",
            dir.path().join("bin").to_string_lossy().into_owned(),
        ),
    ]));
    let mut sandbox = podman_with(dir.path().join("no-such-podman"));
    let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));

    let chosen = group("podman", &sandbox, &paths, &ProbeMode::Cached, &cache);
    let program = chosen.get(BackendCheckId::Program).expect("program");
    assert_eq!(program.status(), CheckStatus::Fail);
    let finding = program.diagnostic().expect("finding");
    assert_eq!(finding.code.as_str(), "SANDBOX_034");
    assert!(finding.fix.is_some(), "{finding:?}");
    assert!(
        chosen.get(BackendCheckId::Probe).is_none(),
        "no probe unless asked"
    );

    sandbox.backend = SandboxBackendName::Auto;
    let other = group("podman", &sandbox, &paths, &ProbeMode::Cached, &cache);
    assert!(!other.selected);
    let program = other.get(BackendCheckId::Program).expect("program");
    assert_eq!(program.status(), CheckStatus::Warn);
    assert!(
        other.checks.iter().all(|c| c.status() != CheckStatus::Fail),
        "{other:?}"
    );
}
