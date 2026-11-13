//! Werkzeug, das nur die Unit-Tests dieser Crate benutzen.
//!
//! Die Integrationstests unter `tests/` sind eigene Crates und sehen dieses
//! Modul nicht; sie haben dieselbe Funktion in `tests/common/mod.rs`.

#[allow(clippy::expect_used)]
/// Legt ein ausführbares Programm an, ohne dass ein Deskriptor dieses
/// Prozesses darauf offen bleibt.
///
/// **Warum nicht einfach `std::fs::write`.** Ein Testbinary hat viele Fäden,
/// und ein `fork` in einem anderen Faden erbt jeden offenen Deskriptor. Fällt
/// er in das Fenster, in dem die Datei hier zum Schreiben offen ist, hält das
/// Kind sie bis zu seinem eigenen `exec` offen, und unser `exec` bekommt
/// `ETXTBSY` -- „Text file busy". `O_CLOEXEC` hilft nicht, weil das Fenster
/// zwischen `fork` und `exec` liegt. Am 2026-09-07 sind die CI und
/// `verify-commit` daran rot geworden
/// (`find_program_reads_path_from_the_given_env_only` und
/// `a_hanging_call_is_cut_off_and_reported_as_a_timeout`), lokal war es unter
/// Last nie zu sehen; die Suche nach der Ursache kostete beide Male dieselbe
/// Zeit (HUM-134).
///
/// Geschrieben wird deshalb über einen eigenen Prozess: Sein Deskriptor gehört
/// ihm, kein anderer Faden dieses Prozesses kann ihn erben, und wenn `cp`
/// zurück ist, hat niemand die Datei mehr zum Schreiben offen.
///
/// Drei Feinheiten, jede aus dem Review dieses Standes:
/// * Der Name der Quelle wird **angehängt** und nicht ersetzt: Mit
///   `with_extension` trügen `test.sh` und `test.py` denselben Quellnamen, und
///   ein `foo.source` wäre seine eigene Quelle -- das `cp` scheiterte, und das
///   Aufräumen löschte das Programm.
/// * Ein vorhandenes Ziel wird vorher entfernt, damit `cp` eine neue Datei
///   anlegt statt die alte zu leeren; die alte könnte gerade ausgeführt
///   werden, und dann wäre `ETXTBSY` zurück.
/// * Die Quelle wird gelöscht, damit im Verzeichnis nur steht, was der Test
///   angelegt hat.
pub(crate) fn write_program(path: &std::path::Path, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;

    let mut name = path.file_name().expect("a file name").to_os_string();
    name.push(".source");
    let source = path.with_file_name(name);
    std::fs::write(&source, body).expect("the source of the program");
    let _ = std::fs::remove_file(path);
    let status = std::process::Command::new("cp")
        .arg(&source)
        .arg(path)
        .status()
        .expect("cp runs");
    assert!(status.success(), "cp failed: {status}");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("the program is executable");
    std::fs::remove_file(&source).expect("the source is gone");
}

/// Ob der Prozess `pid` noch lebt (HUM-457): Er steht in `/proc` und ist
/// kein Zombie. Ein Zombie läuft nicht mehr; er wartet nur darauf, dass sein
/// Elternprozess ihn einsammelt.
#[cfg(target_os = "linux")]
pub(crate) fn process_alive(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // Der Zustand steht nach der Klammer um den Namen, und der Name darf
    // selbst Klammern enthalten: also nach der letzten.
    let state = stat
        .rsplit_once(')')
        .and_then(|(_, rest)| rest.split_whitespace().next());
    !matches!(state, Some("Z" | "X") | None)
}

#[allow(clippy::panic)]
/// Liest eine PID, die eine Attrappe der Engine in `path` schrieb, und
/// wartet darauf höchstens fünf Sekunden (HUM-457).
#[cfg(target_os = "linux")]
pub(crate) fn pid_written_to(path: &std::path::Path) -> u32 {
    let since = std::time::Instant::now();
    loop {
        if let Some(pid) = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse().ok())
        {
            return pid;
        }
        assert!(
            since.elapsed() < std::time::Duration::from_secs(5),
            "no pid in {}",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Der Körper einer Attrappe der Engine, die nie antwortet (HUM-457): Sie
/// startet ein Kind, das ebenso schläft, schreibt ihre eigene PID nach
/// `leader` und die des Kindes nach `child` und wartet dann auf das Kind.
/// Beendet ein Abbruch nur sie und nicht ihre Gruppe, lebt das Kind weiter.
#[cfg(target_os = "linux")]
pub(crate) fn never_answering_engine(leader: &std::path::Path, child: &std::path::Path) -> String {
    format!(
        "#!/bin/sh\nsleep 1000 &\necho $! > '{child}.tmp' && mv '{child}.tmp' '{child}'\n\
         echo $$ > '{leader}.tmp' && mv '{leader}.tmp' '{leader}'\nwait\n",
        child = child.display(),
        leader = leader.display()
    )
}
