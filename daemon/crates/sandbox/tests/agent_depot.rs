//! The agent depot inside a real sandbox (HUM-518), in the pattern of the
//! escape suite: plan with the bwrap backend, start, read what the command
//! saw.
//!
//! The start tests need `bwrap` and unprivileged user namespaces; without
//! them they say so on stderr and end green, like `tests/launcher.rs`. The
//! shim is a stand-in that reports and starts the command; the depot's
//! mounts and masks are the launcher's, not the shim's.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::ffi::OsString;
use std::fmt::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_config::agents::{AgentSource, SourceLock};
use humanitl_config::{Config, Env, Paths, WorkMode};
use humanitl_core::ids::SessionId;
use humanitl_sandbox::agent::depot::{
    DepotInput, DepotPlan, DepotSession, Purpose, ReservedAgent, plan_session,
};
use humanitl_sandbox::{
    BwrapBackend, LaunchOptions, PlanRequest, SandboxBackend, SandboxProfile, SessionContext,
    StdioMode,
};

mod common;

/// Obergrenze für einen Lauf, keine erwartete Dauer. Die Socket-Prüfung aus
/// `README.md` durchsucht das ganze Dateisystem der Sandbox; auf einem
/// CI-Runner mit großem `/usr` und parallel laufenden Tests dauerte das
/// länger als 60 Sekunden.
const WAIT: Duration = Duration::from_secs(300);

const FAKE_SHIM: &str = r#"#!/bin/bash
# fake humanitl-shim for the depot tests: report, then exec. No filter.
if [ "$1" = "--proxy-port" ]; then shift 2; fi
if [ "$1" = "--pins-fd" ]; then shift 2; fi
if [ "$1" = "--" ]; then shift; fi
if [ -n "${HUMANITL_REPORT_FD:-}" ]; then
  fd="$HUMANITL_REPORT_FD"
  echo "CHECK mounts_match ok fake" >&"$fd"
  exec {fd}>&-
fi
exec "$@"
"#;

/// The stand-in for a host install of the agent: if it ever runs, it says so.
const HOST_AGENT: &str = "#!/bin/sh\necho host-agent-ran\n";

struct Fixture {
    dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    socket: PathBuf,
    _listener: UnixListener,
    ca: PathBuf,
    shim: PathBuf,
}

fn private(path: &Path) {
    std::fs::create_dir_all(path).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path();
    let paths = Paths::new(
        Env::from_process()
            .with("XDG_RUNTIME_DIR", base.join("run").to_string_lossy())
            .with("XDG_DATA_HOME", base.join("data").to_string_lossy()),
    );
    let work = base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let socket = paths.proxy_socket();
    private(socket.parent().unwrap());
    let listener = UnixListener::bind(&socket).unwrap();
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
    let ca = base.join("ca.crt");
    std::fs::write(&ca, b"-----BEGIN CERTIFICATE-----\n").unwrap();
    let shim = base.join("humanitl-shim");
    common::write_program(&shim, FAKE_SHIM);
    Fixture {
        paths,
        work,
        socket,
        _listener: listener,
        ca,
        shim,
        dir,
    }
}

impl Fixture {
    /// Claude Code in the depot, filled by hand until HUM-519 installs.
    fn fill_depot(&self) -> PathBuf {
        let depot = self.paths.agents_dir().join("claude");
        let version = depot.join("program/1.0.0");
        for dir in [
            self.paths.agents_dir(),
            depot.clone(),
            depot.join("program"),
            version.clone(),
            depot.join("config"),
            depot.join("credentials"),
        ] {
            private(&dir);
        }
        let program = version.join("claude");
        std::fs::write(&program, "#!/bin/sh\necho depot-program \"$@\"\n").unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
        let manifest = version.join("manifest.json");
        std::fs::write(&manifest, r#"{"version":"1.0.0","signature":"unsigned"}"#).unwrap();
        std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o600)).unwrap();
        let settings = depot.join("config/settings.json");
        std::fs::write(&settings, b"{}\n").unwrap();
        std::fs::set_permissions(&settings, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink("program/1.0.0", depot.join("current")).unwrap();
        depot
    }

    fn context(&self, command: &[OsString], depot: DepotSession) -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: self.work.clone(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: self.socket.clone(),
            ca_cert_src: self.ca.clone(),
            ca_bundle_src: self.ca.clone(),
            shim_src: self.shim.clone(),
            session_env: Vec::new(),
            command: command.to_vec(),
            files: Vec::new(),
            agent_program: None,
            agent_depot: depot,
        }
    }

    fn depot_plan(&self, command: &[&str], from_caller: bool) -> DepotPlan {
        let mut config = Config::default();
        config.agents.claude.source = Some(AgentSource::Depot);
        let command: Vec<OsString> = command.iter().map(OsString::from).collect();
        plan_session(&DepotInput {
            config: &config,
            lock: &SourceLock::none(),
            paths: &self.paths,
            session: SessionId::nil(),
            adapter: "opencode",
            command: &command,
            from_caller,
            purpose: Purpose::Start,
            keys: None,
        })
        .unwrap()
    }

    fn real(&self) -> Option<BwrapBackend> {
        match BwrapBackend::detect(self.paths.clone()) {
            Ok(backend) => Some(backend),
            Err(err) => {
                eprintln!("skipping: bwrap is not usable on this machine: {err}");
                None
            }
        }
    }
}

fn profile() -> SandboxProfile {
    SandboxProfile::load(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml"),
    )
    .unwrap()
}

/// Runs the session and returns its standard output.
fn run(
    backend: &BwrapBackend,
    session: &SessionContext,
    options: &LaunchOptions,
    scratch: Option<humanitl_sandbox::agent::depot::Scratch>,
) -> String {
    let profile = profile();
    let plan = backend
        .plan(&PlanRequest::new(&profile, session, options))
        .expect("plan");
    let handle = backend.launch(&plan).expect("bwrap starts");
    if let Some(scratch) = scratch {
        scratch.hand_to(&handle);
    }
    let status = handle
        .wait_timeout(WAIT)
        .expect("ends in time")
        .unwrap_or_else(|err| panic!("bwrap failed before the command ran: {err}"));
    let output = handle.output().expect("captured");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        status.success(),
        "{status}: {stdout}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    stdout
}

fn capture() -> LaunchOptions {
    LaunchOptions {
        stdio: StdioMode::Capture,
        ..LaunchOptions::default()
    }
}

/// AC2 inside the sandbox: the depot program starts with its arguments, its
/// directory cannot be written, a file the session writes into its
/// configuration stays out of the depot and goes with the session, and the
/// socket check of `README.md` still shows exactly one line.
///
/// Mutation proof: the program directory bound writable turns this red.
#[test]
fn the_depot_program_runs_read_only_and_nothing_stays_in_the_depot() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let depot = fx.fill_depot();
    let plan = fx.depot_plan(&["claude", "--model", "x"], false);
    let scratch_dir = plan
        .session
        .mount
        .as_ref()
        .unwrap()
        .scratch
        .clone()
        .unwrap();
    let mut command = plan.command.clone();
    let probe = concat!(
        "\"$0\" \"$@\"; ",
        "if touch /run/humanitl/agent/planted 2>/dev/null; then echo program-writable; ",
        "else echo program-read-only; fi; ",
        "echo from-session > \"$CLAUDE_CONFIG_DIR/written\" && echo wrote-config; ",
        "echo \"config=$CLAUDE_CONFIG_DIR updates=$DISABLE_UPDATES\"; ",
        "find / \\( -path /proc -o -path /sys -o \\( -path '/dev/*' ! -path /dev/shm ! -path ",
        "'/dev/shm/*' \\) \\) -prune -o -xtype s -print 2>/dev/null | sed 's/^/socket /'"
    );
    command.splice(0..0, ["/bin/sh", "-c", probe].map(OsString::from));
    let session = fx.context(&command, plan.session.clone());
    let stdout = run(&backend, &session, &capture(), plan.scratch);

    assert!(stdout.contains("depot-program --model x"), "{stdout}");
    assert!(stdout.contains("program-read-only"), "{stdout}");
    assert!(stdout.contains("wrote-config"), "{stdout}");
    assert!(
        stdout.contains("config=/run/humanitl/agent-config updates=1"),
        "{stdout}"
    );
    let sockets: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("socket "))
        .collect();
    assert_eq!(sockets, ["socket /run/humanitl/proxy.sock"], "{stdout}");
    assert!(!depot.join("program/1.0.0/planted").exists());
    assert!(
        !depot.join("config/written").exists(),
        "a write reached the depot"
    );
    assert!(!scratch_dir.exists(), "the scratch goes with the session");
}

/// AC3 under a lock: a stand-in host install of every agent `agents.<id>`
/// knows, recipe or not, bound where the sandbox sees it, is an empty
/// read-only mount, so `/bin/sh -c <place>` finds nothing; the host's
/// `$HOME` is not there either.
///
/// Mutation proof: without the mask the stand-ins run and this turns red.
#[test]
fn every_host_install_is_masked_and_the_home_is_not_mounted() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let bin = fx.dir.path().join("usr-local-bin");
    std::fs::create_dir_all(&bin).unwrap();
    let host_home = std::env::var("HOME").unwrap_or_else(|_| "/home".to_owned());
    let mut reserved = Vec::new();
    let mut probe = String::new();
    for agent in humanitl_sandbox::agent::depot::KNOWN {
        let stand_in = bin.join(agent.program);
        common::write_program(&stand_in, HOST_AGENT);
        reserved.push(ReservedAgent {
            agent: agent.id.to_owned(),
            program: agent.program.to_owned(),
            host_installs: vec![stand_in.clone(), bin.join(format!("missing-{}", agent.id))],
            locked_by: Some("/etc/humanitl/policy.toml".to_owned()),
        });
        let _ = write!(
            probe,
            "/bin/sh -c {path} 2>/dev/null; echo \"{id} size=$(wc -c < {path})\"; ",
            path = stand_in.display(),
            id = agent.id,
        );
    }
    let _ = write!(
        probe,
        "if test -e {host_home}; then echo home-visible; else echo home-absent; fi"
    );
    let depot = DepotSession {
        reserved,
        ..DepotSession::default()
    };
    let command: Vec<OsString> = ["/bin/sh", "-c", probe.as_str()]
        .map(OsString::from)
        .to_vec();
    let mut options = capture();
    // The stand-in place, where the sandbox sees it, like `/usr/local/bin`
    // under the profile's read-only `/usr`.
    options.session_ro.push((bin.clone(), bin.clone()));
    let stdout = run(&backend, &fx.context(&command, depot), &options, None);
    assert!(!stdout.contains("host-agent-ran"), "{stdout}");
    for agent in humanitl_sandbox::agent::depot::KNOWN {
        assert!(stdout.contains(&format!("{} size=0", agent.id)), "{stdout}");
    }
    assert!(stdout.contains("home-absent"), "{stdout}");
}

/// AC4: the depot needs no kernel level; a depot session starts under bwrap
/// with `sandbox.min_isolation` at its default, and a missing install
/// location costs no `EROFS`.
#[test]
fn a_depot_session_starts_at_the_shared_level() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    fx.fill_depot();
    assert_eq!(
        humanitl_config::SandboxRef::default().min_isolation,
        humanitl_config::MinIsolation::default()
    );
    let plan = fx.depot_plan(&["claude"], false);
    let session = fx.context(&plan.command, plan.session.clone());
    let stdout = run(&backend, &session, &capture(), plan.scratch);
    assert!(stdout.contains("depot-program"), "{stdout}");
}

/// HUM-566: a package of several files, as the Codex recipe takes it over:
/// `bin/codex` beside its helper `codex-path/rg`, and the daemon's link
/// `codex -> bin/codex` at the top. The bare name starts the program through
/// the link, the program finds its helper beside its real place, and the
/// version directory passes the check before the bind.
///
/// Mutation proof: without the link the session finds no `codex` and this
/// turns red.
#[test]
fn a_package_of_several_files_starts_through_the_daemons_link() {
    let fx = fixture();
    let Some(backend) = fx.real() else { return };
    let depot = fx.paths.agents_dir().join("codex");
    let version = depot.join("program/0.171.0");
    for dir in [
        fx.paths.agents_dir(),
        depot.clone(),
        depot.join("program"),
        version.clone(),
        version.join("bin"),
        version.join("codex-path"),
        depot.join("config"),
        depot.join("credentials"),
    ] {
        private(&dir);
    }
    for (rel, text, mode) in [
        (
            "bin/codex",
            "#!/bin/sh\nhere=$(dirname \"$(readlink -f \"$0\")\")\n\
             echo \"codex-from-depot $* via $0 helper=$(\"$here/../codex-path/rg\")\"\n",
            0o700,
        ),
        ("codex-path/rg", "#!/bin/sh\necho rg-beside\n", 0o700),
        (
            "manifest.json",
            r#"{"version":"0.171.0","signature":"unsigned"}"#,
            0o600,
        ),
    ] {
        std::fs::write(version.join(rel), text).unwrap();
        std::fs::set_permissions(version.join(rel), std::fs::Permissions::from_mode(mode)).unwrap();
    }
    std::os::unix::fs::symlink("bin/codex", version.join("codex")).unwrap();
    std::os::unix::fs::symlink("program/0.171.0", depot.join("current")).unwrap();

    let mut config = Config::default();
    config.agents.codex.source = Some(AgentSource::Depot);
    let command = vec![OsString::from("codex"), OsString::from("hello")];
    let plan = plan_session(&DepotInput {
        config: &config,
        lock: &SourceLock::none(),
        paths: &fx.paths,
        session: SessionId::nil(),
        adapter: "codex",
        command: &command,
        from_caller: false,
        purpose: Purpose::Start,
        keys: None,
    })
    .unwrap();
    assert_eq!(plan.command[0], OsString::from("/run/humanitl/agent/codex"));
    let session = fx.context(&plan.command, plan.session.clone());
    let stdout = run(&backend, &session, &capture(), plan.scratch);
    assert!(
        stdout.contains("codex-from-depot hello via /run/humanitl/agent/codex helper=rg-beside"),
        "{stdout}"
    );
}
