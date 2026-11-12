//! Der Doctor je Backend mit Probestart, gegen die echte Maschine (HUM-436).
//!
//! Der Probestart unter bwrap braucht ein nutzbares bwrap und den gebauten
//! Shim; fehlt eines, sagt der Test das auf stderr und prüft nichts.
//! `cargo test --workspace` baut den Shim vorher, `cargo test -p
//! humanitl-sandbox` allein nicht. Die Container-Engine ist bis HUM-448 nicht
//! zugelassen und startet nie; ihre Tests vor der Zulassung stehen in ihrem
//! Adapter, weil sie die Engine beim Namen nennen.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_config::{Env, Paths, SandboxBackendName, SandboxRef};
use humanitl_sandbox::doctor::{
    BackendCheckId, BackendGroup, CheckStatus, ProbeMode, backend_groups, locate_shim,
};

/// Pfade, deren Daten- und Laufzeitverzeichnis in `dir` liegen, damit kein
/// Zwischenspeicher des Menschen am Rechner gelesen oder geschrieben wird.
fn paths_in(dir: &std::path::Path) -> Paths {
    let runtime = dir.join("runtime");
    std::fs::create_dir_all(&runtime).unwrap();
    Paths::new(
        Env::from_process()
            .with("XDG_DATA_HOME", dir.join("data").to_string_lossy())
            .with("XDG_RUNTIME_DIR", runtime.to_string_lossy()),
    )
}

fn group<'a>(groups: &'a [BackendGroup], name: &str) -> &'a BackendGroup {
    groups
        .iter()
        .find(|group| group.name == name)
        .unwrap_or_else(|| panic!("no group {name}: {groups:?}"))
}

/// Der Probestart unter dem gewählten bwrap ist grün, und ein zweiter Doctor
/// ohne `--probe` zeigt ihn aus dem Zwischenspeicher, ohne zu starten.
#[test]
fn the_probe_start_under_bwrap_is_green_and_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_in(dir.path());
    let shim = locate_shim();
    if !shim.is_file() {
        eprintln!(
            "skipping: {} is not built (cargo build --workspace)",
            shim.display()
        );
        return;
    }
    let sandbox = SandboxRef {
        backend: SandboxBackendName::Bubblewrap,
        ..SandboxRef::default()
    };
    let groups = backend_groups(&sandbox, &paths, &ProbeMode::Start { shim });
    let bwrap = group(&groups, "bwrap");
    assert!(bwrap.selected);
    let available = bwrap.get(BackendCheckId::Available).expect("available");
    if available.status() != CheckStatus::Ok {
        eprintln!(
            "skipping: bwrap is not usable on this machine: {:?}",
            available.diagnostic()
        );
        return;
    }
    let probe = bwrap.get(BackendCheckId::Probe).expect("probe");
    assert_eq!(
        probe.status(),
        CheckStatus::Ok,
        "{} {:?}",
        probe.evidence(),
        probe.diagnostic()
    );

    let again = backend_groups(&sandbox, &paths, &ProbeMode::Cached);
    let cached = group(&again, "bwrap")
        .get(BackendCheckId::Probe)
        .expect("the remembered probe");
    assert_eq!(cached.status(), CheckStatus::Ok);
    assert!(
        cached.evidence().ends_with("(cached)"),
        "{}",
        cached.evidence()
    );
    // Nichts vom Probestart bleibt im Proxy-Verzeichnis liegen.
    let left: Vec<_> = std::fs::read_dir(paths.proxy_socket_dir())
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "{left:?}");
}

/// Ohne `--probe` und ohne gespeicherten Probestart steht keine Zeile
/// `probe` da: Der Doctor startet von sich aus nichts.
#[test]
fn without_probe_nothing_is_started_and_no_probe_line_appears() {
    let dir = tempfile::tempdir().unwrap();
    let paths = paths_in(dir.path());
    let groups = backend_groups(&SandboxRef::default(), &paths, &ProbeMode::Cached);
    assert!(!groups.is_empty());
    for group in &groups {
        assert!(group.get(BackendCheckId::Admitted).is_some(), "{group:?}");
        assert!(group.get(BackendCheckId::Probe).is_none(), "{group:?}");
    }
    assert!(
        !paths.proxy_socket_dir().exists(),
        "nothing was prepared for a start"
    );
}
