//! Der Doctor je Backend für Podman (HUM-436): das falsche Programm. Die
//! Stufe vor ihrer Zulassung prüft seit HUM-464 ein Test in
//! `crate::doctor::backends` am VM-Backend der Kernel-Stufe.
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

/// Ein falsches `sandbox.podman.program` ist in der Gruppe des gewählten
/// Backends `fail` mit `SANDBOX_034` und dem Installationsbefehl; ist
/// Podman nicht gewählt, bleibt dieselbe Zeile `warn`. Seit HUM-448 ist die
/// gemeinsame Stufe zugelassen, und die Zeile ist die der Suche vor jedem
/// Start (`available`).
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
    let program = chosen.get(BackendCheckId::Available).expect("program");
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
    let program = other.get(BackendCheckId::Available).expect("program");
    assert_eq!(program.status(), CheckStatus::Warn);
    assert!(
        other.checks.iter().all(|c| c.status() != CheckStatus::Fail),
        "{other:?}"
    );
}
