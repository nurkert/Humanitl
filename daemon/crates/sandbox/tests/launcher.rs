//! Der Launcher: Plan, Start, Bericht (HUM-011, HUM-013).
//!
//! Die Plan-Tests laufen überall. Die Start-Tests brauchen `bwrap` und einen
//! Kernel, der unprivilegierte Nutzer-Namensräume erlaubt; fehlt eines,
//! sagen sie es auf stderr und enden grün, denn „kein `bwrap` auf dieser
//! Maschine" ist eine Aussage über die Maschine, nicht über den Launcher.
//!
//! Den Shim (HUM-012) ersetzt hier ein Bash-Skript, das den Vertrag aus
//! `humanitl_sandbox::bridge_env` gerade so weit erfüllt, wie der Launcher ihn
//! braucht: es schreibt vier `CHECK`-Zeilen auf `HUMANITL_REPORT_FD`,
//! schließt den Deskriptor und startet den Befehl. Damit ist der Weg vom
//! Plan über `bwrap` bis zum Bericht getestet, ohne dass der Test am Filter
//! hängt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::ids::SessionId;
use humanitl_sandbox::{
    BwrapBackend, ENV_REPORT_FD, IsolationCheck, LaunchPlan, SandboxBackend, SandboxFile,
    SandboxHandle, SandboxProfile, SessionContext, StdioMode, Version,
};

/// Kein Test wartet länger: eine Sandbox, die nach einer Minute noch läuft,
/// ist ein Befund, kein Grund, die Suite anzuhalten.
const WAIT: Duration = Duration::from_secs(60);

/// Ein Shim-Ersatz, der den Bericht schreibt und dann den Befehl startet.
const FAKE_SHIM: &str = r#"#!/bin/bash
# fake humanitl-shim for the launcher tests: report, then exec. No filter.
if [ "$#" -eq 0 ]; then exit 125; fi
if [ "$1" = "--proxy-port" ]; then shift 2; fi
if [ "$1" = "--" ]; then shift; fi
if [ -n "${HUMANITL_REPORT_FD:-}" ]; then
  fd="$HUMANITL_REPORT_FD"
  echo "CHECK bridge_listening ok fake listener on ${HUMANITL_BRIDGES:-?}" >&"$fd"
  echo "CHECK single_socket ok sockets=/run/humanitl/proxy.sock;unexpected=none;entries=7;limit=none" >&"$fd"
  echo "CHECK seccomp_applied fail fake shim installs no filter" >&"$fd"
  echo "CHECK families ok fake ${HUMANITL_SECCOMP_FAMILIES:-?}" >&"$fd"
  echo "CHECK no_interfaces ok lo" >&"$fd"
  exec {fd}>&-
fi
exec "$@"
"#;

/// Ein Shim-Ersatz, der eine zweite Tür findet: die Bridge lauscht, aber im
/// Dateisystem liegt ein Socket, den niemand bestellt hat.
const SECOND_SOCKET_SHIM: &str = r#"#!/bin/bash
# fake humanitl-shim: the bridge answers, but the walk found another socket.
if [ "$#" -eq 0 ]; then exit 125; fi
if [ "$1" = "--proxy-port" ]; then shift 2; fi
if [ "$1" = "--" ]; then shift; fi
if [ -n "${HUMANITL_REPORT_FD:-}" ]; then
  fd="$HUMANITL_REPORT_FD"
  echo "CHECK bridge_listening ok fake listener" >&"$fd"
  echo "CHECK single_socket fail sockets=/run/humanitl/proxy.sock,/work/second.sock;unexpected=/work/second.sock;entries=7;limit=none" >&"$fd"
  echo "CHECK seccomp_applied ok Seccomp:2;NoNewPrivs:1" >&"$fd"
  echo "CHECK families ok fake" >&"$fd"
  echo "CHECK no_interfaces ok lo" >&"$fd"
  exec {fd}>&-
fi
exec "$@"
"#;

/// Ein Shim-Ersatz, der nichts meldet.
const SILENT_SHIM: &str = "#!/bin/sh\nif [ \"$1\" = \"--proxy-port\" ]; then shift 2; fi\nif [ \"$1\" = \"--\" ]; then shift; fi\nexec \"$@\"\n";

struct Fixture {
    _dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    socket: PathBuf,
    _listener: UnixListener,
    ca: PathBuf,
    ca_bundle: PathBuf,
    shim: PathBuf,
    state: PathBuf,
}

fn write_executable(path: &Path, content: &str) {
    std::fs::write(path, content).expect("write script");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn bind_socket(path: &Path) -> UnixListener {
    std::fs::create_dir_all(path.parent().unwrap()).expect("socket dir");
    std::fs::set_permissions(
        path.parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .expect("chmod dir");
    let listener = UnixListener::bind(path).expect("bind placeholder socket");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).expect("chmod socket");
    listener
}

fn fixture_with_shim(shim_script: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let state = dir.path().to_path_buf();
    let runtime = state.join("runtime");
    let paths = Paths::new(Env::from_process().with("XDG_RUNTIME_DIR", runtime.to_string_lossy()));

    let work = state.join("work");
    std::fs::create_dir_all(work.join(".git/hooks")).expect("hooks");
    std::fs::create_dir_all(work.join(".vscode")).expect("vscode");
    std::fs::write(work.join(".git/config"), "[user]\n\tname = canary\n").expect("git config");
    std::fs::write(work.join(".envrc"), "export CANARY=1\n").expect("envrc");

    let socket = paths.proxy_socket();
    let listener = bind_socket(&socket);
    let ca = state.join("ca.crt");
    std::fs::write(&ca, b"-----BEGIN CERTIFICATE-----\n").expect("ca");
    let ca_bundle = state.join("ca-bundle.crt");
    std::fs::write(&ca_bundle, b"-----BEGIN CERTIFICATE-----\n").expect("ca bundle");
    let shim = state.join("humanitl-shim");
    write_executable(&shim, shim_script);

    Fixture {
        _dir: dir,
        paths,
        work,
        socket,
        _listener: listener,
        ca,
        ca_bundle,
        shim,
        state,
    }
}

fn fixture() -> Fixture {
    fixture_with_shim(FAKE_SHIM)
}

fn profile(name: &str) -> SandboxProfile {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles/sandbox")
        .join(format!("{name}.toml"));
    SandboxProfile::load(&path)
        .unwrap_or_else(|err| panic!("{} does not load: {err}", path.display()))
}

fn probe(body: &str) -> SandboxProfile {
    let text = format!("version = 1\nname = \"probe\"\n{body}");
    SandboxProfile::parse(&text, Path::new("<probe>")).expect("the probe profile parses")
}

impl Fixture {
    fn context(&self, command: &[&str]) -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: self.work.clone(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: self.socket.clone(),
            ca_cert_src: self.ca.clone(),
            ca_bundle_src: self.ca_bundle.clone(),
            shim_src: self.shim.clone(),
            session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
            command: command.iter().map(OsString::from).collect(),
            files: Vec::new(),
        }
    }

    /// Ein Backend ohne Prüfung, für die Plan-Tests.
    fn unchecked(&self) -> BwrapBackend {
        BwrapBackend::unchecked("/usr/bin/bwrap", Version(0, 11, 0), self.paths.clone())
    }

    /// Das echte `bwrap`, oder `None` mit Begründung auf stderr.
    fn real(&self) -> Option<BwrapBackend> {
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(backend.with_stdio(StdioMode::Capture)),
            Err(err) => {
                eprintln!("skipping: bwrap is not usable on this machine: {err}");
                None
            }
        }
    }
}

fn strings(args: &[OsString]) -> Vec<String> {
    args.iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

fn window_at(args: &[String], window: &[&str]) -> Option<usize> {
    args.windows(window.len())
        .position(|w| w.iter().zip(window).all(|(a, b)| a == b))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Wartet mit Frist und verlangt, dass der Befehl lief.
fn wait(handle: &SandboxHandle) -> ExitStatus {
    handle
        .wait_timeout(WAIT)
        .expect("the sandbox ends within the timeout")
        .unwrap_or_else(|err| panic!("bwrap failed before the command ran: {err}"))
}

// --- der Plan ----------------------------------------------------------------

#[test]
fn plan_renders_the_shim_and_the_three_binds() {
    let fx = fixture();
    let backend = fx.unchecked();
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&["sh", "-c", "echo hello world"]),
        )
        .expect("the plan builds");
    let args = strings(&plan.argv);

    assert_eq!(args[0], "/usr/bin/bwrap", "argv[0] is the program");
    assert_eq!(args[1], "--unshare-user");
    for (src, dst) in [
        (fx.socket.to_str().unwrap(), "/run/humanitl/proxy.sock"),
        (fx.ca.to_str().unwrap(), "/etc/humanitl/ca.crt"),
        (
            fx.ca_bundle.to_str().unwrap(),
            "/etc/ssl/certs/ca-certificates.crt",
        ),
        (fx.shim.to_str().unwrap(), "/run/humanitl/humanitl-shim"),
    ] {
        window_at(&args, &["--ro-bind", src, dst])
            .unwrap_or_else(|| panic!("--ro-bind {src} {dst} is missing: {args:?}"));
    }
    assert_eq!(
        &args[args.len() - 8..],
        [
            "--",
            "/run/humanitl/humanitl-shim",
            "--proxy-port",
            "3128",
            "--",
            "sh",
            "-c",
            "echo hello world",
        ]
    );

    assert_descriptors(&plan, &args);

    // Nur was im Projekt liegt, wird überdeckt: .git/hooks, .vscode, .envrc
    // und .git/config gibt es, .idea nicht.
    assert!(window_at(&args, &["--tmpfs", "/work/.git/hooks"]).is_some());
    assert!(window_at(&args, &["--tmpfs", "/work/.vscode"]).is_some());
    assert!(window_at(&args, &["--tmpfs", "/work/.idea"]).is_none());
    assert!(
        !args.contains(&"/dev/null".to_owned()),
        "no /dev/null masks"
    );

    // Die Umgebung im Plan ist die der --setenv-Paare, alphabetisch.
    let setenv: Vec<(String, String)> = args
        .windows(3)
        .filter(|w| w[0] == "--setenv")
        .map(|w| (w[1].clone(), w[2].clone()))
        .collect();
    assert_eq!(plan.env, setenv);
    let mut sorted = plan.env.clone();
    sorted.sort();
    assert_eq!(plan.env, sorted);

    // Die Zeile ist die Liste.
    let line = plan.argv_line();
    assert!(line.starts_with("/usr/bin/bwrap --unshare-user"), "{line}");
    assert_eq!(shlex::split(&line).expect("parsable"), args);
    assert!(plan.is_fresh());
    assert_eq!(plan.profile, "default");
    assert_eq!(plan.program(), Path::new("/usr/bin/bwrap"));
}

/// Die Deskriptoren des Plans: Bericht, Status, die drei Identitätsdateien und
/// ein memfd je Maske, die es im Projekt gibt (`.envrc` und `.git/config`).
///
/// Jede Nummer in der Argumentliste ist eine aus `fds`, jede aus `fds` steht
/// in der Liste, und keine kommt doppelt vor: `--ro-bind-data` schließt seinen
/// Deskriptor nach dem Lesen.
fn assert_descriptors(plan: &LaunchPlan, args: &[String]) {
    assert_eq!(plan.fds.len(), 2 + 3 + 2, "{:?}", plan.fds);
    for (host, target) in &plan.fds {
        assert_eq!(host, target, "inherited under its own number");
    }
    let planned: Vec<String> = plan.fds.iter().map(|(fd, _)| fd.to_string()).collect();
    let mut distinct = planned.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        planned.len(),
        "every descriptor once: {planned:?}"
    );

    let status_at = args
        .iter()
        .position(|a| a == "--json-status-fd")
        .expect("--json-status-fd");
    let status_fd = args[status_at + 1].clone();
    let report_fd = args
        .windows(3)
        .find(|w| w[0] == "--setenv" && w[1] == ENV_REPORT_FD)
        .map(|w| w[2].clone())
        .expect("HUMANITL_REPORT_FD is set");
    let data_fd = |dst: &str| -> String {
        let at = args
            .iter()
            .position(|a| a == dst)
            .unwrap_or_else(|| panic!("{dst} is not in the argument list"));
        assert_eq!(
            args[at - 2],
            "--ro-bind-data",
            "{dst} comes from a descriptor"
        );
        args[at - 1].clone()
    };
    let mut used = vec![status_fd, report_fd];
    for dst in [
        "/etc/passwd",
        "/etc/group",
        "/etc/hosts",
        "/work/.envrc",
        "/work/.git/config",
    ] {
        used.push(data_fd(dst));
    }
    let mut used_sorted = used.clone();
    used_sorted.sort();
    used_sorted.dedup();
    assert_eq!(
        used_sorted.len(),
        used.len(),
        "no descriptor is named twice: {used:?}"
    );
    for fd in &used {
        assert!(planned.contains(fd), "{fd} is not in plan.fds {planned:?}");
    }
}

/// Die Dateien des Agent-Adapters kommen als `--ro-bind-data` in die Zeile,
/// jede mit einem eigenen, vererbten Deskriptor.
#[test]
fn plan_carries_the_files_of_the_agent_adapter() {
    let fx = fixture();
    let backend = fx.unchecked();
    let mut context = fx.context(&["sh", "-c", "true"]);
    context.files = vec![
        SandboxFile::read_only("/etc/humanitl/opencode/opencode.json", b"{}".to_vec()),
        SandboxFile::read_only("/etc/humanitl/opencode/models.json", b"{}".to_vec()),
    ];
    let plan = backend
        .plan(&profile("default"), &context)
        .expect("the plan builds");
    let args = strings(&plan.argv);

    assert_eq!(plan.files.len(), 2, "the plan carries the list for the UI");
    let mut seen = Vec::new();
    for dst in [
        "/etc/humanitl/opencode/opencode.json",
        "/etc/humanitl/opencode/models.json",
    ] {
        let at = args
            .iter()
            .position(|arg| arg == dst)
            .unwrap_or_else(|| panic!("{dst} is not in the line: {args:?}"));
        assert_eq!(args[at - 2], "--ro-bind-data");
        let fd: i32 = args[at - 1].parse().expect("a descriptor number");
        assert!(fd >= 0, "{dst} has no descriptor");
        assert!(
            plan.fds
                .iter()
                .any(|(host, target)| *host == fd && *target == fd),
            "the descriptor of {dst} is inherited"
        );
        seen.push(at);
    }
    assert!(seen[0] < seen[1], "the order is the order of the list");
    let clearenv = args
        .iter()
        .position(|arg| arg == "--clearenv")
        .expect("--clearenv is in the line");
    assert!(seen[1] < clearenv, "the files come before --clearenv");
}

/// Nichts vom Adapter darf in das Projektverzeichnis.
#[test]
fn plan_refuses_an_adapter_file_under_work() {
    let fx = fixture();
    let backend = fx.unchecked();
    let mut context = fx.context(&["sh", "-c", "true"]);
    context.files = vec![SandboxFile::read_only(
        "/work/opencode.json",
        b"{}".to_vec(),
    )];

    let err = backend
        .plan(&profile("default"), &context)
        .expect_err("a file under /work is refused");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("/work"), "{}", err.why);
}

#[test]
fn plan_rejects_a_missing_or_unwritable_work_dir() {
    let fx = fixture();
    let backend = fx.unchecked();

    let mut ctx = fx.context(&["true"]);
    ctx.work_src = fx.state.join("nowhere");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a missing project directory");
    assert_eq!(err.code.as_str(), "SANDBOX_005");
    assert!(err.why.contains("does not exist"), "{}", err.why);

    ctx.work_src = fx.ca.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a file is not a project directory");
    assert_eq!(err.code.as_str(), "SANDBOX_005");
    assert!(err.why.contains("not a directory"), "{}", err.why);

    ctx.work_src = PathBuf::from("relative/proj");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("relative paths are ambiguous");
    assert_eq!(err.code.as_str(), "SANDBOX_005");

    // root darf überall schreiben; dann ist der Fall nicht prüfbar.
    if rustix::process::getuid().is_root() {
        eprintln!("skipping the read-only check: running as root");
        return;
    }
    let read_only = fx.state.join("ro-proj");
    std::fs::create_dir(&read_only).expect("mkdir");
    std::fs::set_permissions(&read_only, std::fs::Permissions::from_mode(0o555)).expect("chmod");
    ctx.work_src = read_only.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("rw on a read-only directory");
    assert_eq!(err.code.as_str(), "SANDBOX_005");
    assert!(err.why.contains("not writable"), "{}", err.why);
    assert!(
        matches!(err.fix, Some(humanitl_core::FixAction::RemountReadOnly(ref p)) if *p == read_only),
        "{:?}",
        err.fix
    );
    ctx.work_mode = WorkMode::Ro;
    backend
        .plan(&profile("default"), &ctx)
        .expect("ro on a read-only directory is fine");
}

/// HUM-013: nur eine Socket-Datei aus dem Proxy-Verzeichnis, mit den Rechten
/// des Daemons.
#[test]
fn plan_rejects_a_proxy_socket_that_is_not_the_proxy_socket() {
    let fx = fixture();
    let backend = fx.unchecked();
    let mut ctx = fx.context(&["true"]);

    ctx.proxy_socket_src = fx.ca.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a regular file is no socket");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    assert!(err.why.contains("not a Unix socket"), "{}", err.why);

    ctx.proxy_socket_src = fx.paths.proxy_socket_dir().join("missing.sock");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a missing socket");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    assert!(err.why.contains("missing"), "{}", err.why);

    // Ein echter Socket, aber am falschen Ort: der gRPC-Socket des Daemons.
    let daemon = fx.paths.daemon_socket();
    let _daemon_listener = bind_socket(&daemon);
    ctx.proxy_socket_src = daemon.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("the daemon socket is never mounted");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("outside"), "{}", err.why);

    // Ein Socket irgendwo sonst.
    let elsewhere = fx.state.join("elsewhere.sock");
    let _elsewhere_listener = bind_socket(&elsewhere);
    ctx.proxy_socket_src = elsewhere;
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a socket outside the proxy directory");
    assert_eq!(err.code.as_str(), "SANDBOX_006");

    // Das Proxy-Verzeichnis selbst, oder ein Weg per `..` hinaus.
    ctx.proxy_socket_src = fx.paths.proxy_socket_dir();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("the directory is not the file");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    ctx.proxy_socket_src = fx.paths.proxy_socket_dir().join("..").join("daemon.sock");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("dot-dot does not escape the directory");
    assert_eq!(err.code.as_str(), "SANDBOX_006");

    // Ein Socket, den jeder öffnen darf, ist keine Tür für genau einen.
    std::fs::set_permissions(&fx.socket, std::fs::Permissions::from_mode(0o666)).expect("chmod");
    ctx.proxy_socket_src = fx.socket.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a world-accessible socket");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("0666"), "{}", err.why);
    std::fs::set_permissions(&fx.socket, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    backend
        .plan(&profile("default"), &ctx)
        .expect("the daemon's own socket, 0600, in its own directory");
}

#[test]
fn plan_rejects_a_missing_ca_or_shim() {
    let fx = fixture();
    let backend = fx.unchecked();
    let mut ctx = fx.context(&["true"]);

    ctx.ca_cert_src = fx.state.join("no-ca.crt");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a missing CA file");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    assert!(err.why.contains("CA certificate"), "{}", err.why);
    assert!(err.why.contains("no-ca.crt"), "{}", err.why);
    ctx.ca_cert_src = fx.state.clone();
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a directory is not a certificate");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    ctx.ca_cert_src = fx.ca.clone();

    ctx.shim_src = fx.state.join("no-shim");
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a missing shim");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    assert!(err.why.contains("shim binary"), "{}", err.why);

    let not_executable = fx.state.join("shim.txt");
    std::fs::write(&not_executable, b"not a program").expect("write");
    ctx.shim_src = not_executable;
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("a shim that cannot run");
    assert_eq!(err.code.as_str(), "SANDBOX_011");
    assert!(err.why.contains("not executable"), "{}", err.why);
}

#[test]
fn plan_rejects_an_old_bwrap() {
    let fx = fixture();
    let backend = BwrapBackend::unchecked("/usr/bin/bwrap", Version(0, 7, 9), fx.paths.clone());
    let err = backend
        .plan(&profile("default"), &fx.context(&["true"]))
        .expect_err("0.7.9 is below min_bwrap_version");
    assert_eq!(err.code.as_str(), "SANDBOX_002");
    assert!(err.why.contains("0.7.9"), "{}", err.why);
    assert!(err.why.contains("0.8.0"), "{}", err.why);
    assert!(err.fix.is_some());
}

#[test]
fn plan_rejects_forbidden_mounts_from_profile_and_session() {
    let fx = fixture();
    let backend = fx.unchecked();

    // Das Projektverzeichnis im Laufzeitverzeichnis: existiert, ist verboten.
    let mut ctx = fx.context(&["true"]);
    ctx.work_src = fx.paths.runtime_dir().path;
    let err = backend
        .plan(&profile("default"), &ctx)
        .expect_err("the runtime directory as project");
    assert_eq!(err.code.as_str(), "SANDBOX_006");

    // Eine Profilquelle im Laufzeitverzeichnis.
    let runtime = fx.paths.runtime_dir().path;
    let bad = probe(&format!(
        "[mounts]\nextra_ro = [{:?}]\n",
        runtime.to_str().unwrap()
    ));
    let err = backend
        .plan(&bad, &fx.context(&["true"]))
        .expect_err("the runtime directory as an extra mount");
    assert_eq!(err.code.as_str(), "SANDBOX_006");
    assert!(err.why.contains("mounts.extra_ro"), "{}", err.why);
}

// --- der Start ---------------------------------------------------------------

#[test]
fn launch_true_exits_zero() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(&profile("default"), &fx.context(&["true"]))
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    assert!(handle.pid > 0);
    assert!(!plan.is_fresh(), "a launched plan is spent");
    let status = wait(&handle);
    assert!(status.success(), "{status}");
    let output = handle.output().expect("captured");
    assert!(output.stdout.is_empty());
    assert!(
        handle.child_pid().is_some(),
        "bwrap reported the child pid: {:?}",
        handle.status()
    );
    assert_eq!(handle.status().exit_code, Some(0));
    assert!(handle.argv_display.contains("--unshare-net"));

    let err = backend.launch(&plan).expect_err("a plan launches once");
    assert_eq!(err.code.as_str(), "SANDBOX_012");
    assert!(err.why.contains("already launched"), "{}", err.why);
}

/// Der ESC-2-Befund: `/proc/1/environ` ist die Umgebung von `bwrap`, und die
/// ist leer; der Befehl sieht genau die `--setenv`-Paare.
#[test]
fn launch_env_is_clean() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&[
                "sh",
                "-c",
                "tr '\\0' '\\n' < /proc/1/environ; echo ---MARK---; env",
            ]),
        )
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let status = wait(&handle);
    let output = handle.output().expect("captured");
    assert!(status.success(), "{status}: {}", text(&output.stderr));

    let stdout = text(&output.stdout);
    let (pid1, agent) = stdout.split_once("---MARK---\n").expect("marker");
    assert!(
        pid1.trim().is_empty(),
        "/proc/1/environ must be empty, got: {pid1}"
    );

    let mut seen: Vec<(String, String)> = agent
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    seen.sort();
    let mut planned = plan.env.clone();
    planned.sort();
    // `env` zeigt auch `PWD`, das die Shell setzt; alles andere ist der Plan.
    seen.retain(|(k, _)| !matches!(k.as_str(), "PWD" | "OLDPWD" | "SHLVL" | "_"));
    assert_eq!(
        seen, planned,
        "the agent sees exactly the planned environment"
    );
    assert!(
        !agent.contains("CARGO_MANIFEST_DIR="),
        "a host variable came along: {agent}"
    );
}

#[test]
fn launch_no_interface_no_resolv_hostname() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&[
                "sh",
                "-c",
                concat!(
                    "tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' '; echo ---; ",
                    "if test -e /etc/resolv.conf; then echo resolv-present; else echo resolv-absent; fi; ",
                    "cat /proc/sys/kernel/hostname; ",
                    "cat /sys/class/net/lo/operstate 2>/dev/null || echo no-sysfs; ",
                    "cat /work/.envrc; echo envrc-end; ",
                    "ls /work/.git/hooks | wc -l; ",
                    "grep -c ^ /proc/self/status > /dev/null && grep ^CapEff /proc/self/status"
                ),
            ]),
        )
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let status = wait(&handle);
    let output = handle.output().expect("captured");
    assert!(status.success(), "{status}: {}", text(&output.stderr));
    let stdout = text(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "lo", "only lo: {stdout}");
    assert_eq!(lines[1], "---", "exactly one interface: {stdout}");
    assert_eq!(lines[2], "resolv-absent", "{stdout}");
    assert_eq!(lines[3], "sandbox", "{stdout}");
    assert_ne!(
        lines[4], "down",
        "lo is up (or unknown, or sysfs is absent): {stdout}"
    );
    assert_eq!(
        lines[5], "envrc-end",
        "the masked .envrc reads as empty: {stdout}"
    );
    assert_eq!(lines[6], "0", ".git/hooks is an empty tmpfs: {stdout}");
    assert!(
        lines[7].trim_end_matches('0').ends_with('\t') || lines[7].ends_with("0000000000000000"),
        "no capabilities: {}",
        lines[7]
    );
}

#[test]
fn launch_early_exit_is_a_diagnostic() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let broken = probe("[mounts]\nro = [\"/nonexistent-humanitl-source\"]\n");
    let plan = backend
        .plan(&broken, &fx.context(&["true"]))
        .expect("the policy has nothing against a missing directory");
    // Innerhalb des Fensters meldet `launch` den Befund, auf einer langsamen
    // Maschine erst `wait`; beide sagen dasselbe.
    let err = match backend.launch(&plan) {
        Err(err) => err,
        Ok(handle) => handle
            .wait_timeout(WAIT)
            .expect("the sandbox ends within the timeout")
            .expect_err("bwrap cannot bind a missing source"),
    };
    assert_eq!(err.code.as_str(), "SANDBOX_012", "{err}");
    assert!(
        err.why.contains("before starting the command"),
        "{}",
        err.why
    );
    assert!(
        err.why.contains("nonexistent-humanitl-source"),
        "the stderr excerpt names the source: {}",
        err.why
    );
}

#[test]
fn kill_terminates_the_sandbox() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(&profile("default"), &fx.context(&["sleep", "30"]))
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    assert!(handle.try_wait().is_none(), "still running");
    let started = Instant::now();
    let other = handle.clone();
    handle.terminate(Duration::from_millis(500));
    let status = wait(&other);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "kill took {:?}",
        started.elapsed()
    );
    assert!(
        !status.success(),
        "a killed sandbox does not report success: {status}"
    );
    assert!(
        status.signal().is_some() || status.code().is_some_and(|c| c >= 128),
        "{status}"
    );
    // Ein zweites kill ist ein no-op.
    handle.kill();
}

/// Der Bericht des Shims kommt durch `bwrap` bis zum Handle, und der Agent
/// erbt den Deskriptor nicht.
#[test]
fn isolation_check_reads_the_report() {
    let fx = fixture();
    if !Path::new("/bin/bash").exists() {
        eprintln!("skipping: the fake shim needs /bin/bash");
        return;
    }
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&["sh", "-c", "ls /proc/self/fd"]),
        )
        .expect("plan");
    let report_fd = plan
        .env
        .iter()
        .find(|(k, _)| k == ENV_REPORT_FD)
        .map(|(_, v)| v.clone())
        .expect("report fd");
    let handle = backend.launch(&plan).expect("bwrap starts");

    let results = backend.isolation_check(&handle);
    let status = wait(&handle);
    let output = handle.output().expect("captured");
    assert!(status.success(), "{status}: {}", text(&output.stderr));

    assert_eq!(results.len(), 3);
    let by = |check: IsolationCheck| results.iter().find(|r| r.check == check).unwrap();
    let network = by(IsolationCheck::NoNetworkInterface);
    assert!(network.passed, "{network:?}");
    assert!(network.diagnostic.is_none());
    assert!(network.evidence.contains("no_interfaces ok"), "{network:?}");
    let socket = by(IsolationCheck::SingleSocket);
    assert!(socket.passed, "{socket:?}");
    assert!(
        socket.evidence.contains("proxy"),
        "the evidence carries what the shim saw: {socket:?}"
    );
    // Die zweite Garantie steht auf beiden Zeilen: der Suchlauf zeigt, dass es
    // keine zweite Tür gibt, `bridge_listening`, dass die eine offen ist.
    assert!(socket.evidence.contains("single_socket ok"), "{socket:?}");
    assert!(
        socket.evidence.contains("bridge_listening ok"),
        "{socket:?}"
    );
    let seccomp = by(IsolationCheck::SeccompActive);
    assert!(!seccomp.passed, "{seccomp:?}");
    let diagnostic = seccomp
        .diagnostic
        .as_ref()
        .expect("a failed check has a diagnostic");
    assert_eq!(diagnostic.code.as_str(), "SANDBOX_016");
    assert!(
        diagnostic.why.contains("installs no filter"),
        "{}",
        diagnostic.why
    );

    let report = handle.report();
    assert!(report.is_complete());
    assert_eq!(report.checks.len(), 5);

    let listing = text(&output.stdout);
    let fds: Vec<&str> = listing.lines().collect();
    assert!(
        !fds.contains(&report_fd.as_str()),
        "the agent must not inherit the report descriptor {report_fd}: {fds:?}"
    );
}

#[test]
fn isolation_check_without_a_report_is_sandbox_013() {
    let fx = fixture_with_shim(SILENT_SHIM);
    let Some(backend) = fx.real() else { return };
    let backend = backend.with_report_timeout(Duration::from_millis(300));
    let plan = backend
        .plan(&profile("default"), &fx.context(&["sleep", "2"]))
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let results = backend.isolation_check(&handle);
    handle.kill();
    assert_eq!(results.len(), 3);
    for result in &results {
        assert!(!result.passed, "{result:?}");
        let diagnostic = result.diagnostic.as_ref().expect("diagnostic");
        assert_eq!(diagnostic.code.as_str(), "SANDBOX_013", "{result:?}");
        assert!(result.evidence.contains("no CHECK line"), "{result:?}");
    }
    assert_eq!(
        results.iter().map(|r| r.check).collect::<Vec<_>>(),
        IsolationCheck::ALL.to_vec()
    );
}

/// Ein lauschender Listener allein ist nicht die zweite Garantie: findet der
/// Suchlauf des Shims einen zweiten Socket, ist `SingleSocket` rot, obwohl die
/// Bridge antwortet (Review-Befund vom 2026-09-03).
#[test]
fn a_second_socket_turns_the_second_guarantee_red() {
    let fx = fixture_with_shim(SECOND_SOCKET_SHIM);
    if !Path::new("/bin/bash").exists() {
        eprintln!("skipping: the fake shim needs /bin/bash");
        return;
    }
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(&profile("default"), &fx.context(&["true"]))
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    let results = backend.isolation_check(&handle);
    let _ = wait(&handle);

    let by = |check: IsolationCheck| {
        results
            .iter()
            .find(|r| r.check == check)
            .unwrap_or_else(|| panic!("{check:?} missing"))
    };
    let socket = by(IsolationCheck::SingleSocket);
    assert!(!socket.passed, "{socket:?}");
    let diagnostic = socket.diagnostic.as_ref().expect("a failed check reports");
    assert_eq!(diagnostic.code.as_str(), "SANDBOX_015");
    assert!(diagnostic.why.contains("/work/second.sock"), "{diagnostic}");
    assert!(
        socket.evidence.contains("bridge_listening ok"),
        "the open door stays in the evidence: {socket:?}"
    );
    assert!(by(IsolationCheck::NoNetworkInterface).passed);
    assert!(by(IsolationCheck::SeccompActive).passed);
}

/// Der Start braucht keinen Tokio-Kontext und überlebt das Ende des Threads,
/// der `launch` gerufen hat: `--die-with-parent` hängt am Thread des
/// Backends, nicht am Aufrufer.
#[test]
fn the_sandbox_outlives_the_calling_thread() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let plan = backend
        .plan(
            &profile("default"),
            &fx.context(&["sh", "-c", "sleep 1; echo alive"]),
        )
        .expect("plan");
    let handle = std::thread::spawn(move || backend.launch(&plan).expect("bwrap starts"))
        .join()
        .expect("the calling thread ends first");
    let status = wait(&handle);
    let output = handle.output().expect("captured");
    assert!(status.success(), "{status}: {}", text(&output.stderr));
    assert_eq!(text(&output.stdout).trim(), "alive");
}
