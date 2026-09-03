//! Der Starter des Escape-Harness (HUM-006).
//!
//! Das Harness braucht etwas, das es aufrufen kann, bevor es die Dinge gibt, die
//! es prüft. Dieses Binary ist genau das und nicht mehr: es liest ein
//! Sandbox-Profil mit [`SandboxProfile::load_validated`] gegen die
//! [`MountPolicy`] aus `humanitl_config::Paths` (derselbe Einstieg, den
//! HUM-011 nimmt), baut mit [`SandboxProfile::to_bwrap_args`] aus HUM-010
//! dieselbe Argumentliste, die später auch der echte Launcher baut, und ersetzt
//! sich per `execvp` durch `bwrap`. Es gibt hier keine zweite Übersetzung des
//! Profils: was die Oberfläche unter „Sandbox" anzeigt, ist Argument für
//! Argument das, was hier startet.
//!
//! ```text
//! escape-launch --profile profiles/sandbox/test.toml \
//!               --tests-dir tests/escape \
//!               --work target/escape/work \
//!               -- /bin/sh /tests/escape/esc-1-sockets.sh
//! ```
//!
//! # Zwei Eingriffe in die Argumentliste, beide befristet
//!
//! 1. **Das Testverzeichnis.** `profiles/sandbox/test.toml` nennt in
//!    `mounts.extra_ro` den Pfad `/tests/escape`. Der ist ein Platzhalter: er
//!    steht für den Ort in der Sandbox, nicht für einen Ort auf dem Host. Mit
//!    `--tests-dir` wird die Quelle dieses einen Binds auf das echte Verzeichnis
//!    des Arbeitsbaums gezogen. Das umgeht die Mount-Allowlist bewusst, genau
//!    wie das Projektverzeichnis: beides kommt aus der Sitzung, nicht aus dem
//!    Profil (siehe [`humanitl_sandbox::MountPolicy`]).
//! 2. **Der Shim.** Die Liste endet laut HUM-010 mit
//!    `-- <shim> --proxy-port <port> -- <befehl>`, und der Shim wird zusätzlich
//!    eingehängt. Den gibt es bis HUM-012 nicht; ohne `--shim` werden deshalb
//!    der Bind und das Präfix entfernt, sodass der Befehl direkt hinter dem
//!    ersten `--` steht. Findet sich die erwartete Form nicht, bricht der
//!    Starter mit [`LaunchError::Harness`] ab, statt eine halb
//!    zusammengestrichene Kommandozeile zu starten.
//!
//! Beide Eingriffe verschwinden, sobald HUM-011 und HUM-012 stehen.
//!
//! # Exit-Codes
//!
//! Der Prozess wird im Erfolgsfall durch `bwrap` ersetzt und hat dann dessen
//! Exit-Code. Vorher gibt es nur drei eigene:
//!
//! - `0` — `--print-argv` oder `--help`, es wurde nichts gestartet.
//! - `2` — die Kommandozeile des Starters selbst ist unbrauchbar
//!   (`SANDBOX_012`); die Gebrauchsanweisung steht auf stderr.
//! - `3` — die Sandbox ließ sich nicht starten. Der Befund steht als
//!   [`Diagnostic`] auf stderr: `SANDBOX_001` (kein `bwrap`), `SANDBOX_002`
//!   (zu alt), `CONFIG_001`/`CONFIG_003`/`SANDBOX_006`/`SANDBOX_007` (Profil),
//!   und für die Vorbedingungen des Harness selbst `SANDBOX_010` (die
//!   Argumentliste hat nicht mehr die Form aus HUM-010) oder `SANDBOX_011`
//!   (ein Platzhalter ließ sich nicht anlegen).
//!
//! `run.sh` unterscheidet daran „die Sandbox lief gar nicht" von „die Sandbox
//! lief und eine Probe ist durchgekommen".

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use humanitl_config::{Paths, WorkMode};
use humanitl_core::diagnostics::codes::{
    SANDBOX_001, SANDBOX_002, SANDBOX_010, SANDBOX_011, SANDBOX_012,
};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::{MountPolicy, SandboxProfile, SessionContext};

/// Exit-Code, wenn die Kommandozeile des Starters selbst unbrauchbar ist.
const EXIT_USAGE: u8 = 2;
/// Exit-Code, wenn die Sandbox gar nicht erst startet.
const EXIT_CANNOT_START: u8 = 3;

/// Der Platzhalter, den `profiles/sandbox/test.toml` in `mounts.extra_ro` nennt.
const TESTS_DIR_DST: &str = "/tests/escape";

const USAGE: &str = "\
escape-launch — start the escape-test sandbox (HUM-006)

usage:
  escape-launch --profile FILE [options] -- COMMAND [ARG...]

options:
  --profile FILE        sandbox profile, normally profiles/sandbox/test.toml
  --tests-dir DIR       host directory bound read-only to /tests/escape
  --work DIR            host directory bound to /work (default: STATE/work)
  --state DIR           where placeholders are created (default: TMPDIR/humanitl-escape)
  --proxy-socket PATH   the daemon's proxy socket; without it an empty
                        directory is bound over the socket path, so that
                        ESC-2 exactly_one_socket is red for the right reason
  --ca-cert FILE        the CA certificate (default: an empty placeholder)
  --shim FILE           the humanitl-shim binary (HUM-012); without it the
                        shim is removed from the argument list
  --print-argv          print bwrap and its arguments, one per line, and exit
  -h, --help            this text
";

/// Warum der Starter nicht bis zu `bwrap` gekommen ist.
///
/// Jede Variante trägt einen [`Diagnostic`] mit registriertem Code; die
/// Variante selbst bestimmt den Exit-Code und ob `main` die
/// Gebrauchsanweisung anhängt.
#[derive(Debug)]
enum LaunchError {
    /// Die Kommandozeile des Starters ist unbrauchbar (`SANDBOX_012`); `main`
    /// hängt `USAGE` an und endet mit [`EXIT_USAGE`].
    Usage(Diagnostic),
    /// Eine Vorbedingung des Harness ist nicht erfüllt: die Argumentliste hat
    /// nicht die Form aus HUM-010 (`SANDBOX_010`) oder ein Platzhalter ließ
    /// sich nicht anlegen (`SANDBOX_011`). Endet mit [`EXIT_CANNOT_START`].
    Harness(Diagnostic),
    /// Ein Befund über das Profil oder die Maschine. Endet mit
    /// [`EXIT_CANNOT_START`].
    Diagnostic(Diagnostic),
}

impl LaunchError {
    /// Der Befund hinter dem Fehler.
    const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::Usage(diagnostic) | Self::Harness(diagnostic) | Self::Diagnostic(diagnostic) => {
                diagnostic
            }
        }
    }
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.diagnostic())
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.diagnostic())
    }
}

impl From<Diagnostic> for LaunchError {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::Diagnostic(diagnostic)
    }
}

/// Ein Fehler der Kommandozeile des Starters (`SANDBOX_012`).
fn usage(why: impl Into<String>) -> LaunchError {
    LaunchError::Usage(
        Diagnostic::builder(SANDBOX_012, Severity::Blocking)
            .why(why)
            .build(),
    )
}

/// Die Argumentliste hat nicht die Form, die HUM-010 erzeugt (`SANDBOX_010`).
fn unexpected_argv(why: impl Into<String>) -> LaunchError {
    LaunchError::Harness(
        Diagnostic::builder(SANDBOX_010, Severity::Blocking)
            .why(why)
            .build(),
    )
}

/// Ein Platzhalter ließ sich nicht anlegen (`SANDBOX_011`).
fn placeholder_failed(path: &Path, err: &std::io::Error) -> LaunchError {
    LaunchError::Harness(
        Diagnostic::builder(SANDBOX_011, Severity::Blocking)
            .why(format!("cannot create {}: {err}", path.display()))
            .build(),
    )
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(LaunchError::Usage(diagnostic)) => {
            eprintln!("escape-launch: {diagnostic}\n{USAGE}");
            ExitCode::from(EXIT_USAGE)
        }
        Err(error) => {
            eprintln!("escape-launch: {error}");
            if let Some(FixAction::CopyCommand(command)) = error.diagnostic().fix.as_ref() {
                eprintln!("escape-launch: try: {command}");
            }
            ExitCode::from(EXIT_CANNOT_START)
        }
    }
}

/// Liest die Argumente, baut die Kommandozeile und ersetzt den Prozess.
///
/// Kehrt nur zurück, wenn nichts gestartet wurde (`--help`, `--print-argv`).
fn run() -> Result<(), LaunchError> {
    let Some(args) = Args::parse(std::env::args_os().skip(1))? else {
        print!("{USAGE}");
        return Ok(());
    };

    // Die Politik kommt aus `humanitl_config::Paths`, nie aus `HOME` und
    // `XDG_RUNTIME_DIR` allein: nur so sind `$XDG_CONFIG_HOME/humanitl`,
    // `$XDG_DATA_HOME/humanitl` und der Ersatz des Laufzeitverzeichnisses
    // unter `/run/user` oder `$TMPDIR` geschützt (CONVENTIONS.md 4.11). Die
    // Umgebung des Prozesses wird hier genau einmal gelesen.
    let paths = Paths::from_process();
    let policy = MountPolicy::from_paths(&paths);
    let profile = SandboxProfile::load_validated(&args.profile, &policy)?;

    let state = args.state.clone();
    make_dir(&state)?;
    let work = args.work.clone().unwrap_or_else(|| state.join("work"));
    make_dir(&work)?;

    // Ohne Proxy-Socket ein leeres Verzeichnis: dann liegt an
    // /run/humanitl/proxy.sock kein Socket, und ESC-2 „exactly_one_socket" ist
    // rot, weil die Sandbox keinen hat — nicht, weil die Probe nicht lief.
    let proxy_socket = if let Some(path) = args.proxy_socket.clone() {
        path
    } else {
        let placeholder = state.join("no-proxy-socket");
        make_dir(&placeholder)?;
        placeholder
    };
    let ca_cert = if let Some(path) = args.ca_cert.clone() {
        path
    } else {
        let placeholder = state.join("ca.crt");
        make_file(&placeholder)?;
        placeholder
    };
    // Ohne HUM-012 gibt es keinen Shim. Der Pfad wird trotzdem gebraucht, weil
    // er in der Argumentliste steht, die gleich wieder von ihm befreit wird.
    let shim = args
        .shim
        .clone()
        .unwrap_or_else(|| state.join("humanitl-shim.missing"));

    let context = SessionContext {
        session: SessionId::nil(),
        work_src: work,
        work_mode: WorkMode::Rw,
        proxy_socket_src: proxy_socket,
        ca_cert_src: ca_cert,
        shim_src: shim.clone(),
        command: args.command.clone(),
    };

    let mut bwrap_argv = profile.to_bwrap_args(&context);

    if let Some(tests_dir) = args.tests_dir.as_deref() {
        let tests_dir = absolute(tests_dir);
        if !rebind_source(&mut bwrap_argv, Path::new(TESTS_DIR_DST), &tests_dir) {
            return Err(unexpected_argv(format!(
                "{}: mounts.extra_ro does not name {TESTS_DIR_DST}, so --tests-dir has nothing to point at",
                args.profile.display()
            )));
        }
    }
    if args.shim.is_none() {
        strip_shim(
            &mut bwrap_argv,
            &shim,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )?;
    }

    if args.print_argv {
        println!("bwrap");
        for argument in &bwrap_argv {
            println!("{}", argument.to_string_lossy());
        }
        return Ok(());
    }

    check_bwrap(&profile.sandbox.min_bwrap_version)?;

    // execvp: der Prozess wird ersetzt. Kehrt der Aufruf zurück, ist er
    // gescheitert; einen Erfolgsfall gibt es hier nicht.
    let error = Command::new("bwrap").args(&bwrap_argv).exec();
    Err(Diagnostic::builder(SANDBOX_001, Severity::Blocking)
        .why(format!("cannot execute bwrap: {error}"))
        .build()
        .into())
}

/// Macht aus einem relativen Pfad einen absoluten, ohne Symlinks aufzulösen.
///
/// `bwrap` verlangt absolute Quellen. `std::path::absolute` reicht, weil der
/// Pfad nicht existieren muss und nichts kanonisiert werden soll.
fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Legt ein Verzeichnis an, falls es fehlt (`SANDBOX_011`, wenn nicht).
fn make_dir(path: &Path) -> Result<(), LaunchError> {
    std::fs::create_dir_all(path).map_err(|err| placeholder_failed(path, &err))
}

/// Legt eine leere Datei an, falls sie fehlt (`SANDBOX_011`, wenn nicht).
fn make_file(path: &Path) -> Result<(), LaunchError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        make_dir(parent)?;
    }
    std::fs::write(path, b"").map_err(|err| placeholder_failed(path, &err))
}

/// Prüft, dass `bwrap` da ist und neu genug.
///
/// Fehlt es, ist das kein Fehlschlag der Proben, sondern ein Befund über die
/// Maschine — `run.sh` schreibt ihn als `<error>` in das JUnit-XML und meldet
/// ihn getrennt von einer durchgekommenen Probe.
fn check_bwrap(minimum: &str) -> Result<(), Diagnostic> {
    let output = Command::new("bwrap")
        .arg("--version")
        .output()
        .map_err(|err| {
            Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                .why(format!(
                    "cannot run bwrap: {err}; the escape harness needs bubblewrap on PATH"
                ))
                .fix(FixAction::CopyCommand(
                    "sudo apt-get install -y bubblewrap".to_owned(),
                ))
                .build()
        })?;

    let line = String::from_utf8_lossy(&output.stdout);
    let Some(found) = line.split_whitespace().nth(1) else {
        // Keine erkennbare Version: das ist kein Grund, nicht zu starten.
        return Ok(());
    };
    if version_of(found) < version_of(minimum) {
        return Err(Diagnostic::builder(SANDBOX_002, Severity::Blocking)
            .why(format!(
                "bwrap {found} is older than the profile's min_bwrap_version {minimum}"
            ))
            .build());
    }
    Ok(())
}

/// Eine Versionsangabe als Zahlentripel, für den Vergleich.
///
/// Nicht lesbare Teile zählen als 0. Das genügt: verglichen werden
/// `bubblewrap 0.11.2` und ein `min_bwrap_version` aus dem Profil, beides
/// schlichte Zahlenfolgen.
fn version_of(text: &str) -> (u32, u32, u32) {
    let mut parts = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u32>().unwrap_or(0));
    (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    )
}

/// Zieht die Quelle eines Binds auf einen anderen Host-Pfad.
///
/// Sucht `--ro-bind`/`--bind` mit dem Ziel `dst` und ersetzt die Quelle.
/// Liefert `false`, wenn es den Bind nicht gibt.
fn rebind_source(args: &mut [OsString], dst: &Path, src: &Path) -> bool {
    let bind = OsStr::new("--bind");
    let ro_bind = OsStr::new("--ro-bind");
    for index in 0..args.len().saturating_sub(2) {
        let flag = args[index].as_os_str();
        if (flag == bind || flag == ro_bind) && Path::new(&args[index + 2]) == dst {
            args[index + 1] = src.as_os_str().to_os_string();
            return true;
        }
    }
    false
}

/// Nimmt den Shim aus der Argumentliste: den Bind und das Präfix vor dem Befehl.
///
/// Bis HUM-012 gibt es die Datei nicht, und `bwrap` bräche schon am `--ro-bind`
/// ab. Danach entfällt dieser Eingriff ersatzlos.
fn strip_shim(
    args: &mut Vec<OsString>,
    shim_src: &Path,
    shim_dst: &Path,
    proxy_port: u16,
) -> Result<(), LaunchError> {
    let port = proxy_port.to_string();
    let tail = [
        OsStr::new("--"),
        shim_dst.as_os_str(),
        OsStr::new("--proxy-port"),
        OsStr::new(port.as_str()),
        OsStr::new("--"),
    ];
    let at = find_window(args, &tail).ok_or_else(|| shape_changed("the shim prefix"))?;
    args.drain(at + 1..at + tail.len());

    let bind = [
        OsStr::new("--ro-bind"),
        shim_src.as_os_str(),
        shim_dst.as_os_str(),
    ];
    let at = find_window(args, &bind).ok_or_else(|| shape_changed("the shim bind"))?;
    args.drain(at..at + bind.len());
    Ok(())
}

/// Der Fehler, wenn die Argumentliste nicht mehr die erwartete Form hat.
fn shape_changed(what: &str) -> LaunchError {
    unexpected_argv(format!(
        "{what} is not where HUM-010 puts it; escape-launch will not start a half-edited command line"
    ))
}

/// Die Position der ersten Fundstelle von `needle` in `args`.
fn find_window(args: &[OsString], needle: &[&OsStr]) -> Option<usize> {
    if needle.is_empty() || args.len() < needle.len() {
        return None;
    }
    (0..=args.len() - needle.len()).find(|&start| {
        args[start..start + needle.len()]
            .iter()
            .zip(needle)
            .all(|(have, want)| have.as_os_str() == *want)
    })
}

/// Die Kommandozeile des Starters.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    /// Das Sandbox-Profil, normalerweise `profiles/sandbox/test.toml`.
    profile: PathBuf,
    /// Host-Verzeichnis, das nur lesbar nach `/tests/escape` kommt.
    tests_dir: Option<PathBuf>,
    /// Host-Verzeichnis, das nach `/work` kommt.
    work: Option<PathBuf>,
    /// Wo die Platzhalter für Socket, CA und Arbeitsverzeichnis entstehen.
    state: PathBuf,
    /// Der Proxy-Socket des Daemons, wenn es schon einen gibt.
    proxy_socket: Option<PathBuf>,
    /// Das CA-Zertifikat, wenn es schon eines gibt.
    ca_cert: Option<PathBuf>,
    /// Der Shim (HUM-012); ohne ihn wird er aus der Argumentliste entfernt.
    shim: Option<PathBuf>,
    /// Nur die Kommandozeile ausgeben, nichts starten.
    print_argv: bool,
    /// Der Befehl hinter `--`.
    command: Vec<OsString>,
}

impl Args {
    /// Liest die Argumente. `Ok(None)` heißt: `--help`, nichts zu tun.
    fn parse(input: impl IntoIterator<Item = OsString>) -> Result<Option<Self>, LaunchError> {
        let mut profile: Option<PathBuf> = None;
        let mut tests_dir = None;
        let mut work = None;
        let mut state = None;
        let mut proxy_socket = None;
        let mut ca_cert = None;
        let mut shim = None;
        let mut print_argv = false;
        let mut command = Vec::new();

        let mut rest = input.into_iter();
        while let Some(argument) = rest.next() {
            match argument.to_str() {
                Some("-h" | "--help") => return Ok(None),
                Some("--print-argv") => print_argv = true,
                Some("--") => {
                    command.extend(rest);
                    break;
                }
                Some("--profile") => profile = Some(value(&mut rest, "--profile")?),
                Some("--tests-dir") => tests_dir = Some(value(&mut rest, "--tests-dir")?),
                Some("--work") => work = Some(value(&mut rest, "--work")?),
                Some("--state") => state = Some(value(&mut rest, "--state")?),
                Some("--proxy-socket") => {
                    proxy_socket = Some(value(&mut rest, "--proxy-socket")?);
                }
                Some("--ca-cert") => ca_cert = Some(value(&mut rest, "--ca-cert")?),
                Some("--shim") => shim = Some(value(&mut rest, "--shim")?),
                _ => {
                    return Err(usage(format!(
                        "unknown argument {:?}",
                        argument.to_string_lossy()
                    )));
                }
            }
        }

        let Some(profile) = profile else {
            return Err(usage("--profile is required"));
        };
        if command.is_empty() && !print_argv {
            return Err(usage(
                "no command after --; there would be nothing to run in the sandbox",
            ));
        }

        Ok(Some(Self {
            profile,
            tests_dir,
            work,
            state: state.unwrap_or_else(|| std::env::temp_dir().join("humanitl-escape")),
            proxy_socket,
            ca_cert,
            shim,
            print_argv,
            command,
        }))
    }
}

/// Der Wert hinter einem Flag.
fn value(rest: &mut impl Iterator<Item = OsString>, flag: &str) -> Result<PathBuf, LaunchError> {
    rest.next()
        .map(PathBuf::from)
        .ok_or_else(|| usage(format!("{flag} needs a value")))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn words(input: &[&str]) -> Vec<OsString> {
        input.iter().map(OsString::from).collect()
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|arg| arg.to_str().expect("every argument is UTF-8").to_owned())
            .collect()
    }

    fn test_profile() -> SandboxProfile {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/test.toml");
        SandboxProfile::load(&path)
            .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
    }

    fn context() -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: PathBuf::from("/tmp/escape/work"),
            work_mode: WorkMode::Rw,
            proxy_socket_src: PathBuf::from("/tmp/escape/no-proxy-socket"),
            ca_cert_src: PathBuf::from("/tmp/escape/ca.crt"),
            shim_src: PathBuf::from("/tmp/escape/humanitl-shim.missing"),
            command: words(&["/bin/sh", "/tests/escape/esc-1-sockets.sh"]),
        }
    }

    /// Der Schnappschuss aus HUM-010, um `--tests-dir` erweitert: dieselbe
    /// Liste, nur zeigt die Quelle des einen Binds jetzt in den Arbeitsbaum.
    #[test]
    fn tests_dir_only_moves_the_source_of_that_one_bind() {
        let profile = test_profile();
        let before = profile.to_bwrap_args(&context());
        let mut after = before.clone();

        assert!(rebind_source(
            &mut after,
            Path::new(TESTS_DIR_DST),
            Path::new("/home/u/humanitl/tests/escape"),
        ));

        assert_eq!(
            before.len(),
            after.len(),
            "no argument was added or removed"
        );
        let changed: Vec<usize> = (0..before.len())
            .filter(|&i| before[i] != after[i])
            .collect();
        assert_eq!(
            changed.len(),
            1,
            "exactly one argument changed: {changed:?}"
        );

        let at = changed[0];
        let after = strings(&after);
        assert_eq!(after[at - 1], "--ro-bind");
        assert_eq!(after[at], "/home/u/humanitl/tests/escape");
        assert_eq!(after[at + 1], TESTS_DIR_DST);
    }

    #[test]
    fn tests_dir_reports_a_profile_that_does_not_declare_it() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
        let profile = SandboxProfile::load(&path).expect("the default profile loads");
        let mut argv = profile.to_bwrap_args(&context());
        assert!(
            !rebind_source(
                &mut argv,
                Path::new(TESTS_DIR_DST),
                Path::new("/home/u/humanitl/tests/escape")
            ),
            "the default profile has no /tests/escape and must say so"
        );
    }

    #[test]
    fn without_a_shim_the_command_follows_the_first_dashes() {
        let profile = test_profile();
        let context = context();
        let mut argv = profile.to_bwrap_args(&context);
        strip_shim(
            &mut argv,
            &context.shim_src,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )
        .expect("the argument list has the shape HUM-010 documents");

        let argv = strings(&argv);
        assert_eq!(
            &argv[argv.len() - 3..],
            ["--", "/bin/sh", "/tests/escape/esc-1-sockets.sh"],
        );
        assert!(
            !argv.iter().any(|arg| arg.contains("humanitl-shim")),
            "neither the bind nor the prefix may survive: {argv:?}"
        );
        assert!(
            !argv.iter().any(|arg| arg == "--proxy-port"),
            "the shim's own flag has to go with it: {argv:?}"
        );
        assert_eq!(
            argv.iter().filter(|arg| *arg == "--").count(),
            1,
            "one separator is left, not two: {argv:?}"
        );
    }

    #[test]
    fn stripping_the_shim_twice_is_an_error_not_a_mangled_command_line() {
        let profile = test_profile();
        let context = context();
        let mut argv = profile.to_bwrap_args(&context);
        strip_shim(
            &mut argv,
            &context.shim_src,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )
        .expect("the first pass finds the shim");
        let once = argv.clone();

        let err = strip_shim(
            &mut argv,
            &context.shim_src,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )
        .expect_err("the second pass has nothing left to remove");
        assert!(
            matches!(
                &err,
                LaunchError::Harness(diagnostic)
                    if diagnostic.code == SANDBOX_010 && diagnostic.why.contains("HUM-010")
            ),
            "a harness precondition with its own code: {err}"
        );
        assert_eq!(argv, once, "a failed pass leaves the list untouched");
    }

    #[test]
    fn with_a_shim_the_argument_list_is_the_one_from_hum_010() {
        let profile = test_profile();
        let argv = strings(&profile.to_bwrap_args(&context()));
        assert_eq!(
            &argv[argv.len() - 7..],
            [
                "--",
                "/usr/local/bin/humanitl-shim",
                "--proxy-port",
                "3128",
                "--",
                "/bin/sh",
                "/tests/escape/esc-1-sockets.sh",
            ],
            "HUM-010 keeps the shim in front of the command"
        );
    }

    #[test]
    fn a_command_line_without_profile_or_command_is_refused() {
        let err = Args::parse(words(&["--", "/bin/sh"])).expect_err("--profile is required");
        assert!(
            matches!(&err, LaunchError::Usage(d) if d.why.contains("--profile")),
            "{err}"
        );

        let err = Args::parse(words(&["--profile", "p.toml"]))
            .expect_err("a sandbox without a command has nothing to do");
        assert!(
            matches!(&err, LaunchError::Usage(d) if d.why.contains("no command")),
            "{err}"
        );

        let err = Args::parse(words(&["--profile"])).expect_err("--profile needs a value");
        assert!(
            matches!(&err, LaunchError::Usage(d) if d.why.contains("needs a value")),
            "{err}"
        );

        let err = Args::parse(words(&["--nonsense"])).expect_err("unknown flags are refused");
        assert!(
            matches!(&err, LaunchError::Usage(d) if d.why.contains("unknown argument")),
            "{err}"
        );
        assert_eq!(err.diagnostic().code, SANDBOX_012);
        assert!(
            err.to_string()
                .starts_with("SANDBOX_012: Kommandozeile des Starters ungültig: "),
            "{err}"
        );
    }

    /// Die eigenen Fehler des Starters tragen die Codes aus CONVENTIONS.md
    /// 4.6: `SANDBOX_012` für die Kommandozeile, `SANDBOX_010` für eine
    /// Argumentliste ohne die Form aus HUM-010, `SANDBOX_011` für einen
    /// Platzhalter, der sich nicht anlegen lässt.
    #[test]
    fn harness_errors_carry_their_own_codes() {
        let err = usage("unknown argument");
        assert!(
            matches!(&err, LaunchError::Usage(d) if d.code == SANDBOX_012),
            "{err}"
        );
        assert!(err.to_string().starts_with("SANDBOX_012: "), "{err}");

        let err = shape_changed("the shim bind");
        assert!(
            matches!(&err, LaunchError::Harness(d) if d.code == SANDBOX_010),
            "{err}"
        );
        assert!(
            err.to_string()
                .starts_with("SANDBOX_010: Argumentliste des Starters unerwartet: the shim bind"),
            "{err}"
        );

        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a-file");
        std::fs::write(&file, b"").expect("write");
        let err = make_dir(&file.join("sub")).expect_err("a directory under a file cannot exist");
        assert!(
            matches!(&err, LaunchError::Harness(d) if d.code == SANDBOX_011),
            "{err}"
        );
        assert!(
            err.to_string()
                .starts_with("SANDBOX_011: Platzhalter nicht anlegbar: cannot create "),
            "{err}"
        );
        let err = make_file(&file.join("sub").join("ca.crt"))
            .expect_err("a file under a file cannot exist either");
        assert!(
            matches!(&err, LaunchError::Harness(d) if d.code == SANDBOX_011),
            "{err}"
        );

        let wrapped = LaunchError::from(
            Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                .why("no bwrap")
                .build(),
        );
        assert!(
            wrapped.to_string().starts_with("SANDBOX_001"),
            "a real diagnostic keeps its code: {wrapped}"
        );
        assert!(std::error::Error::source(&wrapped).is_some());
    }

    #[test]
    fn everything_after_the_separator_belongs_to_the_command() {
        let parsed = Args::parse(words(&[
            "--profile",
            "profiles/sandbox/test.toml",
            "--tests-dir",
            "tests/escape",
            "--",
            "/bin/sh",
            "-c",
            "echo --profile is not a flag here",
        ]))
        .expect("the line parses")
        .expect("it is not --help");

        assert_eq!(parsed.profile, PathBuf::from("profiles/sandbox/test.toml"));
        assert_eq!(parsed.tests_dir, Some(PathBuf::from("tests/escape")));
        assert_eq!(
            parsed.command,
            words(&["/bin/sh", "-c", "echo --profile is not a flag here"])
        );
    }

    #[test]
    fn help_asks_for_nothing_else() {
        assert!(
            Args::parse(words(&["--help"]))
                .expect("--help is not an error")
                .is_none()
        );
        assert!(
            Args::parse(words(&["-h"]))
                .expect("-h is not an error")
                .is_none()
        );
    }

    #[test]
    fn print_argv_needs_no_command() {
        let parsed = Args::parse(words(&["--profile", "p.toml", "--print-argv"]))
            .expect("the line parses")
            .expect("it is not --help");
        assert!(parsed.print_argv);
        assert!(parsed.command.is_empty());
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert_eq!(version_of("0.11.2"), (0, 11, 2));
        assert_eq!(version_of("bubblewrap 0.8"), (0, 8, 0));
        assert!(version_of("0.11.2") > version_of("0.8.0"));
        assert!(version_of("0.9") < version_of("0.10"));
        assert_eq!(version_of("nonsense"), (0, 0, 0));
    }
}
