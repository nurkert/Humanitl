//! Die Suche der Engine in Liste und Doctor (HUM-502): eine langsame Engine
//! hält keine Antwort auf, und Werte über diesem Rechner stehen schon in der
//! Liste.
//!
//! Die Tests stehen im Adapter, weil sie Podman beim Namen nennen müssen;
//! außerhalb des Adapters spricht der Code nur vom Backend
//! (`tools/check_coupling.py`).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use humanitl_config::{Env, HostResources, Paths, SandboxBackendName, SandboxRef};

use crate::backend::{BWRAP_BACKEND, PODMAN_BACKEND};
use crate::backend_list::{BackendEntry, entries, list_backends_within};
use crate::backend_search::BackendSearches;
use crate::doctor::{
    BackendCheckId, BackendGroup, CheckStatus, NOT_MEASURED, ProbeMode, backend_groups_within,
};

/// `sleep` mit festem Pfad: Die Engine startet mit einer Umgebung, deren
/// `PATH` nur den Ordner der Attrappen kennt.
fn sleep_program() -> PathBuf {
    ["/bin/sleep", "/usr/bin/sleep"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
        .expect("sleep is installed")
}

/// Ein Ordner mit einer Engine und den Programmen des Dienstverwalters, die
/// je 10 s schlafen: Welcher Weg der Suche auch läuft, er braucht 10 s.
fn slow_engine(dir: &Path) -> Paths {
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let body = format!("#!/bin/sh\nexec {} 10\n", sleep_program().display());
    for name in [
        PODMAN_BACKEND,
        crate::user_unit::RUN_PROGRAM,
        crate::user_unit::CTL_PROGRAM,
    ] {
        crate::test_support::write_program(&bin.join(name), &body);
    }
    Paths::new(Env::from_pairs([
        ("HOME", dir.to_string_lossy().into_owned()),
        ("PATH", bin.to_string_lossy().into_owned()),
        (
            "XDG_RUNTIME_DIR",
            dir.join("run").to_string_lossy().into_owned(),
        ),
    ]))
}

fn codes(diagnostics: &[humanitl_core::Diagnostic]) -> Vec<&str> {
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}

fn entry<'a>(list: &'a [BackendEntry], name: &str) -> &'a BackendEntry {
    list.iter()
        .find(|entry| entry.name == name)
        .unwrap_or_else(|| panic!("{name} is not listed: {list:?}"))
}

fn group<'a>(groups: &'a [BackendGroup], name: &str) -> &'a BackendGroup {
    groups
        .iter()
        .find(|group| group.name == name)
        .unwrap_or_else(|| panic!("{name} has no group: {groups:?}"))
}

/// Eine Engine, deren Suche 10 s braucht, hält weder die Liste noch den
/// Doctor über ihre Frist auf: Die Engine steht als „wird geprüft" da
/// (`DOCTOR_012`, nicht verfügbar, in der Gruppe nie `fail`, auch gewählt),
/// und jedes andere Backend ist vollständig. Mutationsbeweis: ohne eigenen
/// Thread je Suche (Suche im Aufrufer) braucht jede Antwort 10 s.
#[test]
fn a_slow_engine_is_still_being_checked_and_the_rest_is_complete() {
    let dir = tempfile::tempdir().unwrap();
    let paths = slow_engine(dir.path());
    let sandbox = SandboxRef {
        backend: SandboxBackendName::Podman,
        ..SandboxRef::default()
    };
    let searches = BackendSearches::new();

    let started = Instant::now();
    let list = list_backends_within(
        &sandbox,
        "auto",
        &paths,
        &searches,
        Some(started + Duration::from_secs(1)),
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the list waited {:?}",
        started.elapsed()
    );
    let engine = entry(&list, PODMAN_BACKEND);
    assert!(!engine.available(), "{engine:?}");
    assert_eq!(codes(&engine.diagnostics), ["DOCTOR_012"], "{engine:?}");
    let pending = &engine.diagnostics[0];
    assert!(pending.why.contains("still being checked"), "{pending:?}");
    assert!(pending.fix.is_some(), "{pending:?}");
    assert_eq!(codes(&engine.runtimes[0].diagnostics), ["DOCTOR_012"]);
    for other in list.iter().filter(|entry| entry.name != PODMAN_BACKEND) {
        assert!(
            !codes(&other.diagnostics).contains(&"DOCTOR_012"),
            "{other:?}"
        );
    }
    // Ohne bwrap im `PATH` ist dessen Eintrag vollständig: der Befund der
    // Suche, nicht „wird geprüft".
    assert_eq!(
        codes(&entry(&list, BWRAP_BACKEND).diagnostics),
        ["SANDBOX_001"]
    );

    let started = Instant::now();
    let groups = backend_groups_within(
        &sandbox,
        &paths,
        &ProbeMode::Cached,
        &searches,
        Some(started + Duration::from_secs(1)),
    );
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "the doctor waited {:?}",
        started.elapsed()
    );
    let engine = group(&groups, PODMAN_BACKEND);
    assert!(engine.selected);
    let available = engine.get(BackendCheckId::Available).expect("available");
    assert_eq!(available.status(), CheckStatus::Warn, "{available:?}");
    assert!(
        available.evidence().starts_with(NOT_MEASURED)
            && available.evidence().contains("still being checked"),
        "{available:?}"
    );
    assert_eq!(
        available.diagnostic().map(|d| d.code.as_str()),
        Some("DOCTOR_012")
    );
    assert!(engine.get(BackendCheckId::Admitted).is_some());
    for other in groups.iter().filter(|group| group.name != PODMAN_BACKEND) {
        let line = other.get(BackendCheckId::Available).expect("available");
        assert!(!line.evidence().contains("still being checked"), "{line:?}");
    }
}

/// Werte über diesem Rechner machen die Engine in der Liste unverfügbar,
/// mit dem Befund, mit dem ihr Plan sie ablehnte (`CONFIG_003`, HUM-501);
/// ihre Laufzeit ebenso, bwrap nicht. Gesetzt zählt, abgeleitet nie.
/// Mutationsbeweis: ohne die Prüfung in der Liste fehlt der Befund.
#[test]
fn engine_values_beyond_this_host_are_named_in_the_list() {
    let tiny = HostResources {
        memory_mib: Some(1024),
        cpus: Some(1),
    };
    let mut sandbox = SandboxRef::default();
    let list = entries(&sandbox, "auto", tiny, |_| None);
    assert!(entry(&list, PODMAN_BACKEND).available(), "derived values");

    sandbox.podman.cpus = Some(2);
    let list = entries(&sandbox, "auto", tiny, |_| None);
    let engine = entry(&list, PODMAN_BACKEND);
    assert_eq!(codes(&engine.diagnostics), ["CONFIG_003"], "{engine:?}");
    assert!(
        engine.diagnostics[0]
            .why
            .contains("sandbox.podman.cpus = 2"),
        "{:?}",
        engine.diagnostics[0]
    );
    assert_eq!(codes(&engine.runtimes[0].diagnostics), ["CONFIG_003"]);
    assert!(entry(&list, BWRAP_BACKEND).available());

    // Die Liste des Daemons misst am Rechner selbst: ein Wert über jedem.
    sandbox.podman.cpus = Some(u32::MAX);
    let list = crate::backend_list::list_backends_with(&sandbox, "auto", |_| None);
    assert!(codes(&entry(&list, PODMAN_BACKEND).diagnostics).contains(&"CONFIG_003"));
}
