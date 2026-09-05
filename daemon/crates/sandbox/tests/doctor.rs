//! Der Bericht des Doctors als Ganzes (HUM-075).
//!
//! Die einzelnen Urteile prüft `doctor::checks`; hier steht, was über alle elf
//! zugleich gilt und was ein Test je Prüfung nicht sieht:
//!
//! - Der Bericht hat genau eine Zeile je [`CheckId`], in der Reihenfolge der
//!   Anzeige. Eine vergessene Prüfung fiele sonst niemandem auf.
//! - Jede nicht-grüne Zeile trägt einen Befund mit `why` und `fix`; jede grüne
//!   trägt keinen. Das sind zwei der vier Akzeptanzkriterien des Issues.
//! - Jeder Code stammt aus dem Bereich `DOCTOR_`, und die Stufe passt zum
//!   Zustand.
//!
//! Die Tatsachen werden gebaut, nicht gemessen: Kein Test hier braucht einen
//! Rechner ohne `bwrap`, ohne seccomp oder mit voller Platte.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::time::Duration;

use humanitl_core::{Severity, diagnostics};
use humanitl_sandbox::Version;
use humanitl_sandbox::doctor::{
    AgentFacts, BwrapFacts, CheckId, CheckOutcome, CheckStatus, CommandRun, DaemonFacts, DiskFacts,
    DoctorReport, LlmFacts, MachineFacts, NOT_MEASURED, PROBE_LLM_COMMAND, Reading, RendererFacts,
    RunOutcome, RuntimeDirFacts, SeccompFacts, SeccompLine, SystemdFacts, TrayFacts, UsernsFacts,
    run,
};

/// So lang darf ein Beleg werden, damit er in eine Tabellenzeile passt.
///
/// Keine Zusage des Vertrags, sondern eine des Auges: Der Beleg steht in einer
/// Spalte neben zehn anderen, und was breiter ist als ein Terminal, schiebt
/// die Tabelle auseinander. Die Erklärung gehört in den Befund.
const EVIDENCE_MAX_CHARS: usize = 160;

/// Die Namensraum-Probe, wie sie auf einem Rechner wirklich aussieht.
///
/// Wortgleich mit dem, was `doctor::probe` startet — zwanzig Argumente. Ein
/// kurzes `bwrap --unshare-user -- /bin/true` stünde hier bequemer und wäre
/// wertlos: Der Befund von `userns` wurde am 2026-09-05 mitten im Wort
/// abgeschnitten, weil die ganze Zeile in seinem Grund stand, und eine
/// Vorgabe, die kürzer ist als die Wirklichkeit, hätte das nie gezeigt.
fn real_userns_command() -> String {
    "/usr/bin/bwrap --unshare-user --unshare-pid --unshare-net --die-with-parent \
     --ro-bind /usr /usr --ro-bind-try /bin /bin --ro-bind-try /lib /lib \
     --ro-bind-try /lib64 /lib64 --ro-bind-try /etc/ld.so.cache /etc/ld.so.cache \
     -- /bin/true"
        .to_owned()
}

/// Eine Zeile in Woerter, fuer die Vorgaben dieser Datei.
fn words(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_owned).collect()
}

/// Eine Maschine, auf der alles stimmt.
fn healthy() -> MachineFacts {
    MachineFacts {
        bwrap: BwrapFacts::Found {
            program: PathBuf::from("/usr/bin/bwrap"),
            version: Version(0, 11, 0),
        },
        userns: UsernsFacts {
            probe: Reading::Found(CommandRun::new(
                words("/usr/bin/bwrap --unshare-user -- /bin/true"),
                RunOutcome::Exited(0),
                String::new(),
                String::new(),
            )),
            apparmor_restrict: Reading::Found("0".to_owned()),
            userns_clone: Reading::Found("1".to_owned()),
        },
        seccomp: SeccompFacts {
            line: Reading::Found(SeccompLine::Present("0".to_owned())),
            kernel_release: Reading::Found("6.1.0-18-amd64".to_owned()),
        },
        runtime_dir: RuntimeDirFacts::Present {
            path: PathBuf::from("/run/user/1000"),
            mode: 0o700,
            owner_uid: 1000,
            our_uid: 1000,
            is_dir: true,
        },
        systemd_user: SystemdFacts {
            state: Reading::Found(CommandRun::new(
                words("systemctl --user is-system-running"),
                RunOutcome::Exited(0),
                "running".to_owned(),
                String::new(),
            )),
            searched: "/usr/bin".to_owned(),
        },
        daemon: DaemonFacts::Reachable {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            version: "0.0.0".to_owned(),
            proto: (1, 4),
            expected_proto: (1, 4),
        },
        agent: AgentFacts::Found {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            program: PathBuf::from("/usr/local/bin/opencode"),
            version: Reading::Found("1.18.25".to_owned()),
        },
        llm: LlmFacts::Answered {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            flavor: "ollama".to_owned(),
            models: 4,
            latency_ms: 9,
            diagnostics: Vec::new(),
        },
        tray: TrayFacts {
            library: Some(PathBuf::from("/usr/lib/libayatana-appindicator3.so.1")),
            readable_dirs: 4,
            searched_dirs: 4,
            desktop: Some("KDE".to_owned()),
        },
        renderer: RendererFacts {
            session_type: Some("wayland".to_owned()),
            nvidia: Reading::Found(false),
            flutter_engine: None,
        },
        disk: DiskFacts::Measured {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            available_bytes: 40 * 1024 * 1024 * 1024,
        },
    }
}

/// Eine Maschine, auf der nichts stimmt.
fn broken() -> MachineFacts {
    MachineFacts {
        bwrap: BwrapFacts::Missing {
            searched: "/usr/bin:/bin".to_owned(),
        },
        userns: UsernsFacts {
            probe: Reading::Found(CommandRun::new(
                words(&real_userns_command()),
                RunOutcome::TimedOut(Duration::from_secs(2)),
                String::new(),
                "bwrap: setting up uid map: Permission denied".to_owned(),
            )),
            apparmor_restrict: Reading::Found("1".to_owned()),
            userns_clone: Reading::Absent,
        },
        seccomp: SeccompFacts {
            line: Reading::Found(SeccompLine::Missing),
            kernel_release: Reading::Found("4.9.0".to_owned()),
        },
        runtime_dir: RuntimeDirFacts::Unset {
            expected: PathBuf::from("/run/user/1000"),
        },
        systemd_user: SystemdFacts {
            state: Reading::Absent,
            searched: "/usr/bin".to_owned(),
        },
        daemon: DaemonFacts::Unreachable {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            diagnostic: Box::new(
                humanitl_core::Diagnostic::builder(
                    diagnostics::codes::DAEMON_001,
                    Severity::Blocking,
                )
                .why("no socket")
                .build(),
            ),
        },
        agent: AgentFacts::Missing {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            searched: "/usr/bin".to_owned(),
            install: "curl -fsSL https://opencode.ai/install | bash".to_owned(),
        },
        llm: LlmFacts::NotContacted {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            command: PROBE_LLM_COMMAND.to_owned(),
        },
        tray: TrayFacts {
            library: None,
            readable_dirs: 4,
            searched_dirs: 4,
            desktop: Some("ubuntu:GNOME".to_owned()),
        },
        renderer: RendererFacts {
            session_type: Some("wayland".to_owned()),
            nvidia: Reading::Found(true),
            flutter_engine: None,
        },
        disk: DiskFacts::Measured {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            available_bytes: 4 * 1024 * 1024,
        },
    }
}

/// Eine Maschine, auf der sich nichts lesen ließ.
fn blind() -> MachineFacts {
    MachineFacts {
        bwrap: BwrapFacts::Unreadable {
            program: PathBuf::from("/usr/bin/bwrap"),
            error: "EACCES".to_owned(),
        },
        userns: UsernsFacts {
            probe: Reading::Absent,
            apparmor_restrict: Reading::Unreadable("EACCES".to_owned()),
            userns_clone: Reading::Unreadable("EACCES".to_owned()),
        },
        seccomp: SeccompFacts {
            line: Reading::Unreadable("EACCES".to_owned()),
            kernel_release: Reading::Unreadable("EACCES".to_owned()),
        },
        runtime_dir: RuntimeDirFacts::Unreadable {
            path: PathBuf::from("/run/user/1000"),
            error: "EACCES".to_owned(),
        },
        systemd_user: SystemdFacts {
            state: Reading::Unreadable("EACCES".to_owned()),
            searched: "/usr/bin".to_owned(),
        },
        daemon: DaemonFacts::NotTried {
            socket: PathBuf::from("/run/user/1000/humanitl/daemon.sock"),
            why: "nobody tried".to_owned(),
        },
        agent: AgentFacts::Found {
            adapter: "opencode".to_owned(),
            command: "opencode".to_owned(),
            program: PathBuf::from("/usr/local/bin/opencode"),
            version: Reading::Unreadable("EACCES".to_owned()),
        },
        llm: LlmFacts::NotContacted {
            endpoint: "http://192.168.1.50:11434".to_owned(),
            command: PROBE_LLM_COMMAND.to_owned(),
        },
        tray: TrayFacts {
            library: None,
            readable_dirs: 0,
            searched_dirs: 6,
            desktop: None,
        },
        renderer: RendererFacts {
            session_type: Some("wayland".to_owned()),
            nvidia: Reading::Unreadable("EACCES".to_owned()),
            flutter_engine: None,
        },
        disk: DiskFacts::Unreadable {
            path: PathBuf::from("/home/u/.local/share/humanitl"),
            error: "EACCES".to_owned(),
        },
    }
}

/// Prüft die Zusagen, die für jede Zeile jedes Berichts gelten.
fn assert_report_invariants(report: &DoctorReport) {
    let ids: Vec<CheckId> = report.checks.iter().map(CheckOutcome::id).collect();
    assert_eq!(
        ids,
        CheckId::ALL.to_vec(),
        "one line per check, in the order of the display"
    );

    for check in &report.checks {
        let id = check.id();
        assert!(!check.evidence().is_empty(), "{id} has no evidence");
        match check.status() {
            CheckStatus::Ok => {
                assert!(check.diagnostic().is_none(), "{id} is green with a finding");
                assert!(
                    !check.is_unmeasured(),
                    "{id} is green without a measurement"
                );
            }
            CheckStatus::Warn | CheckStatus::Fail => {
                let diagnostic = check
                    .diagnostic()
                    .unwrap_or_else(|| panic!("{id} is not green and carries no finding"));
                assert!(!diagnostic.why.is_empty(), "{id} has no why");
                assert!(diagnostic.fix.is_some(), "{id} has no fix");
                // Auf dem Weg zur Oberfläche geht jedes `why` durch
                // `sanitize_note` und wird dort auf `NOTE_MAX_CHARS` gekappt.
                // Ein längerer Grund kommt beim Menschen mitten im Wort
                // abgeschnitten an — genau das stand am 2026-09-05 in der
                // Zeile `userns`, weil die ganze bwrap-Kommandozeile im Grund
                // stand. Der Beleg ist enger gefasst: Er soll in eine Spalte
                // passen.
                assert!(
                    diagnostic.why.chars().count() <= humanitl_core::block::NOTE_MAX_CHARS,
                    "{id}: the why is {} characters and the contract carries {}: {}",
                    diagnostic.why.chars().count(),
                    humanitl_core::block::NOTE_MAX_CHARS,
                    diagnostic.why
                );
                assert!(
                    check.evidence().chars().count() <= EVIDENCE_MAX_CHARS,
                    "{id}: the evidence is {} characters and a table cell holds {EVIDENCE_MAX_CHARS}: {}",
                    check.evidence().chars().count(),
                    check.evidence()
                );
                let code = diagnostic.code.as_str();
                assert!(code.starts_with("DOCTOR_"), "{id} carries {code}");
                assert!(
                    diagnostics::lookup(diagnostic.code).is_some(),
                    "{code} is not in the register"
                );
                let severity = diagnostic.severity;
                if check.status() == CheckStatus::Fail {
                    assert_eq!(severity, Severity::Blocking, "{id} fails without blocking");
                } else {
                    assert!(
                        severity == Severity::Warning || severity == Severity::Info,
                        "{id} warns with {severity}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_healthy_machine_is_green_everywhere_and_says_what_it_measured() {
    let report = run(&healthy());
    assert_report_invariants(&report);
    assert_eq!(report.worst(), CheckStatus::Ok, "{report:?}");
    assert!(!report.has_failure());
    for check in &report.checks {
        assert!(
            !check.evidence().starts_with(NOT_MEASURED),
            "{} claims to be green without looking: {}",
            check.id(),
            check.evidence()
        );
    }
}

#[test]
fn a_broken_machine_names_every_defect_with_a_way_out() {
    let report = run(&broken());
    assert_report_invariants(&report);
    assert!(report.has_failure());
    assert_eq!(report.worst(), CheckStatus::Fail);

    // Die vier, ohne die nichts startet, sind `fail` und nicht bloß `warn`.
    for id in [
        CheckId::Bwrap,
        CheckId::Userns,
        CheckId::Seccomp,
        CheckId::RuntimeDir,
    ] {
        let check = report.get(id).unwrap_or_else(|| panic!("the line {id}"));
        assert_eq!(check.status(), CheckStatus::Fail, "{id}: {check:?}");
    }
    // Die anderen halten den Start nicht auf.
    for id in [
        CheckId::SystemdUser,
        CheckId::Daemon,
        CheckId::Agent,
        CheckId::Llm,
        CheckId::Tray,
        CheckId::Renderer,
        CheckId::DiskSpace,
    ] {
        let check = report.get(id).unwrap_or_else(|| panic!("the line {id}"));
        assert_eq!(check.status(), CheckStatus::Warn, "{id}: {check:?}");
    }
}

#[test]
fn a_machine_that_could_not_be_read_is_never_reported_as_healthy() {
    let report = run(&blind());
    assert_report_invariants(&report);
    assert_ne!(report.worst(), CheckStatus::Ok, "{report:?}");

    // Keine einzige Zeile ist grün: Grün hieße hier „ich habe nachgesehen",
    // und nachgesehen hat der Doctor nirgends.
    for check in &report.checks {
        assert_ne!(
            check.status(),
            CheckStatus::Ok,
            "{} is green although nothing could be read: {}",
            check.id(),
            check.evidence()
        );
    }

    // Neun der elf sind ausdrücklich „nicht gemessen". Die beiden anderen
    // wurden gemessen — an den Tatsachen des Aufrufers, die es hier gibt:
    // niemand hat den Daemon versucht (auch nicht gemessen), und der Endpunkt
    // ist bekannt, aber nicht angesprochen.
    let unmeasured = report
        .checks
        .iter()
        .filter(|check| check.is_unmeasured())
        .count();
    assert_eq!(unmeasured, CheckId::ALL.len() - 1, "{report:?}");
    let llm = report.get(CheckId::Llm).expect("the llm line");
    assert_eq!(
        llm.diagnostic().map(|diagnostic| diagnostic.code.as_str()),
        Some("DOCTOR_013"),
        "not measured for a reason of its own"
    );
}

#[test]
fn every_check_carries_its_own_code_and_no_code_is_shared() {
    let broken = run(&broken());
    let mut codes: Vec<(CheckId, &str)> = Vec::new();
    for check in &broken.checks {
        if let Some(diagnostic) = check.diagnostic() {
            codes.push((check.id(), diagnostic.code.as_str()));
        }
    }
    // `DOCTOR_012` und `DOCTOR_013` gehören keiner Prüfung, sondern dem
    // Umstand, dass nicht gemessen wurde; alle anderen Codes stehen für genau
    // eine Zeile.
    for (id, code) in &codes {
        if *code == "DOCTOR_012" || *code == "DOCTOR_013" {
            continue;
        }
        let others: Vec<CheckId> = codes
            .iter()
            .filter(|(other_id, other_code)| other_id != id && other_code == code)
            .map(|(other_id, _)| *other_id)
            .collect();
        assert!(
            others.is_empty(),
            "{code} belongs to {id} and to {others:?}"
        );
    }
}

#[test]
fn the_report_of_a_machine_where_nothing_answers_is_still_eleven_lines() {
    for facts in [healthy(), broken(), blind()] {
        assert_eq!(run(&facts).checks.len(), CheckId::ALL.len());
    }
}
