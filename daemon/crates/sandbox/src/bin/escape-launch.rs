//! Der Starter des Escape-Harness (HUM-006, auf HUM-011 umgestellt).
//!
//! Das Harness braucht etwas, das es aufrufen kann. Dieses Binary ist genau
//! das und nicht mehr: es liest ein Sandbox-Profil mit
//! [`SandboxProfile::load_validated`] gegen die [`MountPolicy`] aus
//! `humanitl_config::Paths`, plant und startet mit dem echten
//! [`BwrapBackend`] (derselbe Weg, den später der Daemon nimmt) und endet mit
//! dem Status der Sandbox. Es gibt hier keine zweite Übersetzung des Profils:
//! was die Oberfläche unter „Sandbox" anzeigt, ist Argument für Argument das,
//! was hier startet.
//!
//! ```text
//! escape-launch --profile profiles/sandbox/test.toml \
//!               --tests-dir tests/escape \
//!               --work target/escape/work \
//!               --state target/escape/state \
//!               -- /bin/sh /tests/escape/esc-1-sockets.sh
//! ```
//!
//! # Was das Harness bereitstellt, bevor Daemon und Proxy existieren
//!
//! - **Der Proxy-Socket.** Ohne `--proxy-socket` bindet der Starter einen
//!   Platzhalter, einen gebundenen, unbenutzten Unix-Socket, dort, wo
//!   `humanitl_config::Paths::proxy_socket` ihn erwartet, mit
//!   `XDG_RUNTIME_DIR` auf `<state>/runtime` umgebogen, damit kein laufender
//!   Daemon berührt wird. ESC-2 zählt damit genau einen Socket, den Proxy;
//!   ESC-3 sieht hinter ihm niemanden, bis HUM-015 den Proxy liefert.
//! - **Das CA-Zertifikat.** Ohne `--ca-cert` eine leere Datei unter `<state>`.
//! - **Der Shim.** `--shim FILE`, sonst `humanitl-shim` neben diesem Binary
//!   (derselbe `target/debug`). Ein Shim, der ohne Argumente nicht mit 125
//!   endet (der Gebrauchsfehler des Vertrags in `humanitl_sandbox::bridge_env`),
//!   ist ein Platzhalter aus Sprint 0 und zählt nicht.
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
//! 2. **Ohne Shim.** Fehlt ein brauchbarer Shim (nicht gebaut, oder ein
//!    Platzhalter), bekommt der Plan einen Platzhalter-Shim, und Bind und Präfix
//!    werden anschließend aus der Liste entfernt, sodass der Befehl direkt
//!    hinter dem ersten `--` steht. Findet sich die erwartete Form nicht,
//!    bricht der Starter mit [`LaunchError::Harness`] ab, statt eine halb
//!    zusammengestrichene Kommandozeile zu starten. Alles, was vom Filter
//!    abhängt, ist dann rot, und der Starter sagt es auf stderr.
//!
//! # Exit-Codes
//!
//! - `0` — `--print-argv` oder `--help`, es wurde nichts gestartet; sonst der
//!   Status der Sandbox (Signal: 128 + Nummer).
//! - `2` — die Kommandozeile des Starters selbst ist unbrauchbar
//!   (`SANDBOX_012`); die Gebrauchsanweisung steht auf stderr.
//! - `3` — die Sandbox ließ sich nicht starten, oder sie lief ohne belegte
//!   Isolation und wurde deshalb sofort beendet. Der Befund steht als
//!   [`Diagnostic`] auf stderr: `SANDBOX_001` (kein `bwrap`), `SANDBOX_002`
//!   (zu alt), `SANDBOX_003` (keine Nutzer-Namensräume),
//!   `CONFIG_001`/`CONFIG_003`/`SANDBOX_006`/`SANDBOX_007` (Profil),
//!   `SANDBOX_005`/`SANDBOX_011`/`SANDBOX_012` (Start),
//!   `SANDBOX_013`/`SANDBOX_014`/`SANDBOX_015`/`SANDBOX_016` (Isolation-Check
//!   ohne Bericht oder rot), und für die Vorbedingungen des Harness selbst
//!   `SANDBOX_010` (die Argumentliste hat nicht mehr die Form aus HUM-010)
//!   oder `SANDBOX_011` (ein Platzhalter ließ sich nicht anlegen).
//!
//! Der Starter läuft damit fail-closed: mit einem Shim, der den Vertrag kennt,
//! wird der Befehl in der Sandbox nur ausgeführt, wenn alle drei Garantien aus
//! der laufenden Sandbox belegt sind.
//!
//! `run.sh` unterscheidet daran „die Sandbox lief gar nicht" von „die Sandbox
//! lief und eine Probe ist durchgekommen".

#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use humanitl_config::{DIR_MODE, Env, FILE_MODE, Paths, WorkMode};
use humanitl_core::diagnostics::codes::{SANDBOX_010, SANDBOX_011, SANDBOX_012, SANDBOX_013};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_sandbox::{
    BwrapBackend, EXIT_USAGE as SHIM_EXIT_USAGE, MIN_BWRAP_VERSION, MountPolicy, SandboxBackend,
    SandboxHandle, SandboxProfile, SessionContext, StdioMode,
};

/// Exit-Code, wenn die Kommandozeile des Starters selbst unbrauchbar ist.
const EXIT_USAGE: u8 = 2;
/// Exit-Code, wenn die Sandbox gar nicht erst startet.
const EXIT_CANNOT_START: u8 = 3;

/// Der Platzhalter, den `profiles/sandbox/test.toml` in `mounts.extra_ro` nennt.
const TESTS_DIR_DST: &str = "/tests/escape";

/// Der Name des Shim-Binaries neben diesem Starter.
const SHIM_BINARY: &str = "humanitl-shim";

/// Das Bundle des Hosts, das ohne `--ca-bundle` in die Sandbox kommt.
const HOST_CA_BUNDLE: &str = "/etc/ssl/certs/ca-certificates.crt";

const USAGE: &str = "\
escape-launch — start the escape-test sandbox (HUM-006, HUM-011)

usage:
  escape-launch --profile FILE [options] -- COMMAND [ARG...]

options:
  --profile FILE        sandbox profile, normally profiles/sandbox/test.toml
  --tests-dir DIR       host directory bound read-only to /tests/escape
  --work DIR            host directory bound to /work (default: STATE/work)
  --state DIR           where placeholders are created (default: TMPDIR/humanitl-escape);
                        XDG_RUNTIME_DIR is pointed at STATE/runtime
  --proxy-socket PATH   the daemon's proxy socket; without it a bound, unused
                        placeholder socket is created where Paths::proxy_socket
                        expects it, so that ESC-2 counts exactly one socket
  --ca-cert FILE        the CA certificate (default: an empty placeholder)
  --ca-bundle FILE      the generated bundle bound over the system trust store
                        (default: the host's own bundle, else an empty placeholder)
  --shim FILE           the humanitl-shim binary (default: next to this binary);
                        without a usable one the shim is removed from the
                        argument list and every seccomp probe stays red
  --print-argv          print the launch plan, one argument per line, and exit
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
    /// Ein Befund über das Profil, die Maschine oder den Start. Endet mit
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
        Ok(code) => code,
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

/// Liest die Argumente, plant, startet und wartet.
///
/// Liefert den Exit-Code der Sandbox, oder `0` ohne Start (`--help`,
/// `--print-argv`).
fn run() -> Result<ExitCode, LaunchError> {
    let Some(args) = Args::parse(std::env::args_os().skip(1))? else {
        print!("{USAGE}");
        return Ok(ExitCode::SUCCESS);
    };

    let state = absolute(&args.state);
    make_dir(&state)?;

    // Die Politik kommt aus `humanitl_config::Paths`, nie aus `HOME` und
    // `XDG_RUNTIME_DIR` allein: nur so sind `$XDG_CONFIG_HOME/humanitl`,
    // `$XDG_DATA_HOME/humanitl` und der Ersatz des Laufzeitverzeichnisses
    // geschützt (CONVENTIONS.md 4.11). Die Umgebung des Prozesses wird hier
    // genau einmal gelesen; das Laufzeitverzeichnis zeigt auf den
    // Zustand des Harness, damit der Platzhalter-Socket nie den eines
    // laufenden Daemons ersetzt.
    let runtime = state.join("runtime");
    let paths = Paths::new(Env::from_process().with("XDG_RUNTIME_DIR", runtime.to_string_lossy()));
    let policy = MountPolicy::from_paths(&paths);
    let profile = SandboxProfile::load_validated(&args.profile, &policy)?;

    let work = args
        .work
        .as_deref()
        .map_or_else(|| state.join("work"), absolute);
    make_dir(&work)?;

    // Der Platzhalter lebt so lange wie dieser Prozess; die Sandbox sieht die
    // gebundene Datei, und dahinter antwortet niemand.
    let (proxy_socket, _placeholder) = if let Some(path) = args.proxy_socket.as_deref() {
        (absolute(path), None)
    } else {
        let path = paths.proxy_socket();
        let listener = bind_placeholder_socket(&path)?;
        (path, Some(listener))
    };
    let (ca_cert, ca_bundle) = ca_sources(&args, &state)?;
    let shim = if let Some(path) = find_shim(args.shim.as_deref()) {
        Shim::Real(path)
    } else {
        let placeholder = state.join("humanitl-shim.placeholder");
        make_file(&placeholder, b"#!/bin/sh\nexit 126\n")?;
        make_executable(&placeholder)?;
        Shim::Placeholder(placeholder)
    };

    let context = SessionContext {
        session: SessionId::nil(),
        work_src: work,
        work_mode: WorkMode::Rw,
        proxy_socket_src: proxy_socket,
        ca_cert_src: ca_cert,
        ca_bundle_src: ca_bundle,
        // Ohne Daemon gibt es kein Env-Kit; das Profil bringt dieselben Paare
        // unter `[env]` mit (`humanitl_proxy::ca::ENV_KIT`).
        session_env: Vec::new(),
        shim_src: shim.path().to_path_buf(),
        command: args.command.clone(),
        files: Vec::new(),
    };

    let backend = match BwrapBackend::detect(paths) {
        Ok(backend) => backend,
        // Die Vorschau braucht kein `bwrap`; der Start schon.
        Err(_) if args.print_argv => {
            BwrapBackend::unchecked("bwrap", MIN_BWRAP_VERSION, Paths::new(Env::from_process()))
        }
        Err(diagnostic) => return Err(diagnostic.into()),
    }
    .with_stdio(StdioMode::Inherit);

    let mut plan = backend.plan(&profile, &context)?;

    if let Some(tests_dir) = args.tests_dir.as_deref() {
        let tests_dir = absolute(tests_dir);
        if !rebind_source(&mut plan.argv, Path::new(TESTS_DIR_DST), &tests_dir) {
            return Err(unexpected_argv(format!(
                "{}: mounts.extra_ro does not name {TESTS_DIR_DST}, so --tests-dir has nothing to point at",
                args.profile.display()
            )));
        }
    }
    if let Shim::Placeholder(path) = &shim {
        eprintln!(
            "escape-launch: no usable {SHIM_BINARY} (pass --shim FILE or build HUM-012); running without the shim, every seccomp probe stays red"
        );
        strip_shim(
            &mut plan.argv,
            path,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )?;
    }

    if args.print_argv {
        for argument in &plan.argv {
            println!("{}", argument.to_string_lossy());
        }
        return Ok(ExitCode::SUCCESS);
    }

    let handle = backend.launch(&plan)?;
    if matches!(shim, Shim::Real(_)) {
        enforce_isolation(&backend, &handle)?;
    }
    let status = handle.wait()?;
    Ok(exit_code_of(status))
}

/// Die CA und das Bundle der Sitzung, beide als Host-Pfad.
///
/// Ohne `--ca-cert` eine leere Datei unter `<state>`: das Harness braucht
/// etwas, das `bwrap` einhängen kann, und der Inhalt zählt erst, wenn der
/// Proxy TLS spricht (HUM-015).
///
/// Das Bundle überdeckt in der Sandbox den System-Vertrauensspeicher
/// ([`humanitl_sandbox::CA_BUNDLE_DST`], HUM-014). Ohne den Proxy gibt es noch
/// keines, das die eigene CA enthielte; dann nimmt das Harness das Bundle des
/// Hosts, damit die Sandbox dieselben Wurzeln sieht wie ohne die Überdeckung,
/// und sonst eine leere Datei, damit `bwrap` überhaupt einen Mountpoint
/// bekommt.
fn ca_sources(args: &Args, state: &Path) -> Result<(PathBuf, PathBuf), LaunchError> {
    let ca_cert = if let Some(path) = args.ca_cert.as_deref() {
        absolute(path)
    } else {
        let placeholder = state.join("ca.crt");
        make_file(&placeholder, b"")?;
        placeholder
    };
    let ca_bundle = if let Some(path) = args.ca_bundle.as_deref() {
        absolute(path)
    } else if Path::new(HOST_CA_BUNDLE).is_file() {
        PathBuf::from(HOST_CA_BUNDLE)
    } else {
        let placeholder = state.join("ca-bundle.crt");
        make_file(&placeholder, b"")?;
        placeholder
    };
    Ok((ca_cert, ca_bundle))
}

/// Der Bericht des Shims als Beleg ins Protokoll des Laufs, und der Abbruch,
/// wenn er eine Garantie nicht belegt.
///
/// Fehlt eine Garantie oder ist sie rot, läuft der Befehl nicht. Eine Sandbox,
/// deren Isolation nicht belegt ist, ist keine Sandbox, und ein Escape-Test in
/// ihr misst nichts: er meldete grün, weil die Probe nicht durchkam, obwohl
/// niemand weiß, ob sie es gekonnt hätte. Deshalb beendet der Starter hier die
/// Sandbox und gibt den Befund zurück, statt nur eine Zeile zu schreiben
/// (Review-Befund vom 2026-09-03). Die Proben selbst messen dasselbe noch
/// einmal von innen.
fn enforce_isolation(backend: &BwrapBackend, handle: &SandboxHandle) -> Result<(), LaunchError> {
    let results = backend.isolation_check(handle);
    for result in &results {
        eprintln!(
            "escape-launch: check {} {}: {}",
            result.check.as_str(),
            if result.passed { "pass" } else { "FAIL" },
            result.evidence
        );
    }
    let Some(failed) = results.iter().find(|result| !result.passed) else {
        return Ok(());
    };
    handle.kill();
    let _ = handle.wait();
    Err(LaunchError::Diagnostic(
        failed.diagnostic.clone().unwrap_or_else(|| {
            Diagnostic::builder(SANDBOX_013, Severity::Blocking)
                .why(format!(
                    "isolation check {} failed without a diagnostic: {}",
                    failed.check.as_str(),
                    failed.evidence
                ))
                .build()
        }),
    ))
}

/// Der Shim, mit dem der Plan gebaut wird.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Shim {
    /// Ein Shim, der den Vertrag erfüllt.
    Real(PathBuf),
    /// Eine ausführbare Datei, damit `plan` sie einhängen kann; wird danach
    /// aus der Liste entfernt.
    Placeholder(PathBuf),
}

impl Shim {
    fn path(&self) -> &Path {
        match self {
            Self::Real(path) | Self::Placeholder(path) => path,
        }
    }
}

/// Der Exit-Code der Sandbox als eigener: der Code, oder 128 + Signal.
fn exit_code_of(status: std::process::ExitStatus) -> ExitCode {
    if let Some(code) = status.code() {
        return ExitCode::from(u8::try_from(code.rem_euclid(256)).unwrap_or(1));
    }
    if let Some(signal) = status.signal() {
        return ExitCode::from(u8::try_from(128 + signal.rem_euclid(128)).unwrap_or(1));
    }
    ExitCode::from(1)
}

/// Sucht den Shim: `--shim`, sonst neben diesem Binary; und prüft, dass er
/// den Vertrag kennt (ohne Argumente Exit [`SHIM_EXIT_USAGE`]).
fn find_shim(explicit: Option<&Path>) -> Option<PathBuf> {
    let candidate = match explicit {
        Some(path) => absolute(path),
        None => std::env::current_exe().ok()?.parent()?.join(SHIM_BINARY),
    };
    if !candidate.is_file() {
        return None;
    }
    shim_speaks_the_contract(&candidate).then_some(candidate)
}

/// Ein Shim aus HUM-012 endet ohne Argumente mit 125 (Gebrauchsfehler); der
/// Platzhalter aus Sprint 0 druckt seine Version und endet mit 0.
fn shim_speaks_the_contract(shim: &Path) -> bool {
    Command::new(shim)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.code() == Some(SHIM_EXIT_USAGE))
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

/// Legt eine Datei mit Inhalt an, falls sie fehlt (`SANDBOX_011`, wenn nicht).
fn make_file(path: &Path, content: &[u8]) -> Result<(), LaunchError> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        make_dir(parent)?;
    }
    std::fs::write(path, content).map_err(|err| placeholder_failed(path, &err))
}

fn make_executable(path: &Path) -> Result<(), LaunchError> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|err| placeholder_failed(path, &err))
}

/// Ein gebundener, unbenutzter Unix-Socket: Verzeichnis `0700`, Datei `0600`,
/// eine alte Datei gleichen Namens wird ersetzt.
fn bind_placeholder_socket(path: &Path) -> Result<UnixListener, LaunchError> {
    let Some(dir) = path.parent() else {
        return Err(placeholder_failed(
            path,
            &std::io::Error::other("the socket path has no parent directory"),
        ));
    };
    make_dir(dir)?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(DIR_MODE))
        .map_err(|err| placeholder_failed(dir, &err))?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|err| placeholder_failed(path, &err))?;
    }
    let listener = UnixListener::bind(path).map_err(|err| placeholder_failed(path, &err))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(FILE_MODE))
        .map_err(|err| placeholder_failed(path, &err))?;
    Ok(listener)
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
/// Solange kein Shim da ist, bräche `bwrap` sonst am `--ro-bind` ab oder
/// der Platzhalter bekäme den Befehl. Danach entfällt dieser Eingriff ersatzlos.
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
    /// Das erzeugte CA-Bundle, wenn es schon eines gibt.
    ca_bundle: Option<PathBuf>,
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
        let mut ca_bundle = None;
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
                Some("--ca-bundle") => ca_bundle = Some(value(&mut rest, "--ca-bundle")?),
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
            ca_bundle,
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

    use std::os::unix::process::ExitStatusExt;

    use humanitl_sandbox::{LaunchInputs, LaunchPlan, Version};

    use super::*;

    /// Ein Plan ist eine Argumentliste; die Eingriffe arbeiten auf `plan.argv`.
    fn argv_of(plan: &LaunchPlan) -> Vec<String> {
        plan.argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

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
            proxy_socket_src: PathBuf::from("/tmp/escape/runtime/humanitl/proxy/proxy.sock"),
            ca_cert_src: PathBuf::from("/tmp/escape/ca.crt"),
            ca_bundle_src: PathBuf::from("/tmp/escape/ca-bundle.crt"),
            shim_src: PathBuf::from("/tmp/escape/humanitl-shim.placeholder"),
            session_env: Vec::new(),
            command: words(&["/bin/sh", "/tests/escape/esc-1-sockets.sh"]),
            files: Vec::new(),
        }
    }

    fn argv() -> Vec<OsString> {
        test_profile().to_bwrap_args(&context(), &LaunchInputs::preview())
    }

    /// Der Schnappschuss aus HUM-010, um `--tests-dir` erweitert: dieselbe
    /// Liste, nur zeigt die Quelle des einen Binds jetzt in den Arbeitsbaum.
    #[test]
    fn tests_dir_only_moves_the_source_of_that_one_bind() {
        let before = argv();
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
        let mut argv = profile.to_bwrap_args(&context(), &LaunchInputs::preview());
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
        let mut argv = argv();
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
        let mut argv = argv();
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
        let argv = strings(&argv());
        assert_eq!(
            &argv[argv.len() - 7..],
            [
                "--",
                "/run/humanitl/humanitl-shim",
                "--proxy-port",
                "3128",
                "--",
                "/bin/sh",
                "/tests/escape/esc-1-sockets.sh",
            ],
            "HUM-010 keeps the shim in front of the command"
        );
    }

    /// Ein Plan des Backends hat das Programm vorn; die Eingriffe des Harness
    /// arbeiten auf `plan.argv` dahinter.
    #[test]
    fn the_harness_edits_apply_to_a_real_plan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().join("state");
        let runtime = state.join("runtime");
        let paths = Paths::new(
            Env::from_pairs([("HOME", "/home/nobody")])
                .with("XDG_RUNTIME_DIR", runtime.to_string_lossy()),
        );
        let work = dir.path().join("work");
        make_dir(&work).expect("work");
        let socket = paths.proxy_socket();
        let _listener = bind_placeholder_socket(&socket).expect("placeholder socket");
        assert_eq!(
            std::fs::metadata(socket.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            DIR_MODE
        );
        assert_eq!(
            std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
            FILE_MODE
        );
        let ca = state.join("ca.crt");
        make_file(&ca, b"").expect("ca");
        let shim = state.join("humanitl-shim.placeholder");
        make_file(&shim, b"#!/bin/sh\nexit 126\n").expect("shim");
        make_executable(&shim).expect("chmod");
        assert!(
            !shim_speaks_the_contract(&shim),
            "exit 126 is not the usage code"
        );

        let profile = test_profile();
        let context = SessionContext {
            session: SessionId::nil(),
            work_src: work,
            work_mode: WorkMode::Rw,
            proxy_socket_src: socket,
            ca_cert_src: ca.clone(),
            ca_bundle_src: ca,
            shim_src: shim.clone(),
            session_env: Vec::new(),
            command: words(&["/bin/sh", "-c", "true"]),
            files: Vec::new(),
        };
        let backend = BwrapBackend::unchecked("/usr/bin/bwrap", Version(0, 11, 0), paths);
        let mut plan = backend.plan(&profile, &context).expect("plan");
        assert_eq!(argv_of(&plan)[0], "/usr/bin/bwrap");

        assert!(rebind_source(
            &mut plan.argv,
            Path::new(TESTS_DIR_DST),
            Path::new("/home/u/humanitl/tests/escape")
        ));
        strip_shim(
            &mut plan.argv,
            &shim,
            &profile.network.shim_dst,
            profile.network.proxy_port,
        )
        .expect("strip");
        let argv = argv_of(&plan);
        assert_eq!(&argv[argv.len() - 4..], ["--", "/bin/sh", "-c", "true"]);
        assert!(!argv.iter().any(|arg| arg.contains("humanitl-shim")));
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
        let err = make_file(&file.join("sub").join("ca.crt"), b"")
            .expect_err("a file under a file cannot exist either");
        assert!(
            matches!(&err, LaunchError::Harness(d) if d.code == SANDBOX_011),
            "{err}"
        );
        let err = bind_placeholder_socket(&file.join("sub").join("proxy.sock"))
            .expect_err("a socket under a file cannot exist either");
        assert!(
            matches!(&err, LaunchError::Harness(d) if d.code == SANDBOX_011),
            "{err}"
        );

        let wrapped = LaunchError::from(
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::SANDBOX_001,
                Severity::Blocking,
            )
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
    fn exit_codes_follow_the_shell_convention() {
        assert_eq!(
            exit_code_of(std::process::ExitStatus::from_raw(0)),
            ExitCode::SUCCESS
        );
        assert_eq!(
            exit_code_of(std::process::ExitStatus::from_raw(3 << 8)),
            ExitCode::from(3)
        );
        // Signal 9 (SIGKILL) => 137.
        assert_eq!(
            exit_code_of(std::process::ExitStatus::from_raw(9)),
            ExitCode::from(137)
        );
    }
}
