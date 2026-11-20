//! Der Wartungslauf des Agent-Depots von Ende zu Ende (HUM-563).
//!
//! Ein echter Lauf im Sandbox-Backend des Systems hinter dem echten Proxy dieser Crate-Familie:
//! Halte-Warteschlange, Regelspeicher und Frage-Pipeline wie im Daemon, nur
//! dass der Weg hinaus ein Stellvertreter ist. Er löst jeden Namen auf eine
//! Adresse aus dem Dokumentationsnetz auf und verbindet jede Anfrage mit
//! einem Stellvertreter-Server im Test, der TLS mit einem Blatt der CA des
//! Proxys spricht. Nichts verlässt den Rechner, kein echter Agent wird
//! geladen.
//!
//! Das Rezept ist ein Stellvertreter für Claude Code ohne Signatur
//! (`SandboxPorts::with_depot_agents`): Es lädt die Version und das Programm
//! von `downloads.standin.test` und fragt einmal `foreign.standin.test`, das
//! nicht im Rezept steht.
//!
//! Belegt wird:
//!
//! 1. Die Installation läuft über die Regeln des Rezepts: Der Server sieht
//!    beide Anfragen, das Depot hält danach `program/2.0.0/claude`, und
//!    `current` nennt die Version.
//! 2. Ein fremder Host wird gehalten: `foreign.standin.test` erscheint als
//!    gehaltener Flow und erreicht den Server nie.
//! 3. Nach dem Lauf steht keine Regel des Laufs mehr im Speicher, und eine
//!    Folgesitzung (Quelle `depot`) startet das neue Programm: Es meldet
//!    seine Version, sein Ordner ist nur lesbar (ein Selbst-Update scheitert),
//!    und derselbe Host des Rezepts wird in ihr gehalten statt erlaubt.
//!
//! Mutationsbeweise (`backlog/sprint-7.md`, HUM-563): ohne `commit` nach dem
//! Ende bleibt `current` leer und der Test ist rot; ohne das Entfernen der
//! Sitzungsregeln erreicht die Folgesitzung den Host ohne Halt, und der Test
//! ist rot.
//!
//! Dazu die Anmeldung mit Rückschrift (HUM-520) und Claude Code durch die
//! ganze Kette mit dem Rezept des Daemons, signiert mit einem Schlüssel des
//! Tests (HUM-522, am Test selbst beschrieben).
//!
//! Braucht das Sandbox-Backend des Systems, unprivilegierte Nutzer-Namensräume, `curl` im `/usr` des
//! Hosts und den gebauten Shim, für HUM-522 auch `gpg`; fehlt etwas, sagt er es und endet grün,
//! unter `CI` rot.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use humanitl_config::agents::AgentSource;
use humanitl_config::{Config, Env, HoldConfig, Limits, Paths, RecorderConfig};
use humanitl_core::{Authority, Diagnostic, FlowEvent, HostName, SessionId};
use humanitl_ipc::sandbox::SandboxPorts;
use humanitl_ipc::session::SessionResolver;
use humanitl_ipc::{SandboxService, v1};
use humanitl_proxy::ca::{CaStore, LeafCache};
use humanitl_proxy::rules_store::{Origin, RulesStore};
use humanitl_proxy::{
    AskPipeline, AsyncStream, ClientTls, ConnectionContext, Egress, FlowHandler, FlowPipeline,
    HandlerPorts, HoldQueue, ProxyCore, ProxyLimits, ResolveError, Resolver, RulesPipeline,
    SessionSettings, SessionState, TrustAnchors, Upstream,
};
use humanitl_sandbox::agent::depot::credential::{
    CredentialField, CredentialFormat, Expiry, NamedFields, TimeUnit, ValueForm,
};
use humanitl_sandbox::agent::depot::{
    Depot, DepotRecipe, HostPlatform, InstallHost, InstallRecipe, KnownAgent, LoginFlow,
    LoginRecipe, MaintenanceTask,
};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_stream::StreamExt as _;

/// Der Host des Rezepts.
const RECIPE_HOST: &str = "downloads.standin.test";
/// Ein Host, den das Rezept nicht nennt.
const FOREIGN_HOST: &str = "foreign.standin.test";
/// Die Version, die der Stellvertreter-Server als aktuelle nennt.
const VERSION: &str = "2.0.0";
/// Wie lange ein gehaltener Flow wartet, bevor er abläuft.
const HOLD_SECS: u64 = 3;

/// Das Rezept: Version und Programm vom Host des Rezepts, dazwischen ein
/// Versuch bei einem fremden Host, der gehalten werden muss.
const SCRIPT: &str = r#"base=https://downloads.standin.test
out=$HUMANITL_DEPOT_OUT
version=$(curl -fsS --max-time 30 "$base/latest")
curl -s -o /dev/null --max-time 30 https://foreign.standin.test/ || true
mkdir -p "$out/program"
curl -fsS --max-time 30 -o "$out/program/claude" "$base/$version/claude"
chmod 0700 "$out/program/claude"
printf '%s\n' "$version" > "$out/version"
"#;

/// Das Programm, das der Server ausliefert: Es nennt seine Version,
/// versucht ein Selbst-Update in seinen eigenen Ordner und fragt den Host
/// des Rezepts, der in einer gewöhnlichen Sitzung gehalten wird.
const PROGRAM: &str = r#"#!/bin/sh
echo "standin-version 2.0.0"
if (: > /run/humanitl/agent/self-update) 2>/dev/null; then
    echo "self-update: written"
else
    echo "self-update: refused"
fi
code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 https://downloads.standin.test/latest)
echo "recipe-host: $code"
"#;

static STAND_IN: [KnownAgent; 1] = [KnownAgent {
    id: "claude",
    display_name: "Claude Code (stand-in)",
    program: "claude",
    host_installs: &["/usr/bin/claude", "/usr/local/bin/claude"],
    recipe: Some(DepotRecipe {
        program: "claude",
        config_env: Some("CLAUDE_CONFIG_DIR"),
        update_off: &[("DISABLE_UPDATES", "1")],
        credential_file: None,
        install: Some(InstallRecipe {
            script: SCRIPT,
            hosts: &[InstallHost {
                host: RECIPE_HOST,
                evidence: "the stand-in server of this test",
            }],
            program: "claude",
            platforms: &[
                (HostPlatform::LinuxX64, "linux-x64"),
                (HostPlatform::LinuxArm64, "linux-arm64"),
                (HostPlatform::LinuxX64Musl, "linux-x64-musl"),
                (HostPlatform::LinuxArm64Musl, "linux-arm64-musl"),
            ],
            signature: None,
            source: "https://downloads.standin.test",
        }),
        login: LoginFlow::PastedCode,
        credential_format: None,
        login_run: None,
        command_keys: &[],
    }),
}];

/// Ein Namensdienst, der jeden Namen auf eine Adresse des
/// Dokumentationsnetzes auflöst; verbunden wird ohnehin nur mit dem
/// Stellvertreter.
struct AnyName;

impl Resolver for AnyName {
    fn resolve<'life0, 'life1, 'async_trait>(
        &'life0 self,
        _host: &'life1 str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<IpAddr>, ResolveError>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(async { Ok(vec![IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))]) })
    }
}

/// Der Stellvertreter-Server: TLS mit einem Blatt der CA des Proxys, HTTP/1.1,
/// eine Antwort je Verbindung. Er zählt, welche Hosts ihn erreicht haben.
#[derive(Clone)]
struct StandIn {
    leaves: Arc<LeafCache>,
    seen: Arc<Mutex<Vec<String>>>,
    /// Further answers by `host/path`, set by a test before its run
    /// (HUM-522: the vendor's release bucket).
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
}

impl StandIn {
    fn hosts(&self) -> Vec<String> {
        self.seen.lock().unwrap().clone()
    }

    async fn serve(self, io: tokio::io::DuplexStream, host: HostName) {
        let Ok(config) = self.leaves.server_config(&host) else {
            return;
        };
        // Nur HTTP/1.1: Die Antwort unten ist von Hand geschrieben.
        let mut config = (*config).clone();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let Ok((mut tls, _)) = humanitl_proxy::tls::accept(Arc::new(config), io).await else {
            return;
        };
        let mut head = Vec::new();
        let mut byte = [0_u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            match tls.read(&mut byte).await {
                Ok(1) => head.push(byte[0]),
                _ => return,
            }
        }
        let text = String::from_utf8_lossy(&head).into_owned();
        let path = text.split_whitespace().nth(1).unwrap_or("/").to_owned();
        self.seen.lock().unwrap().push(format!("{host}{path}"));
        let served = self
            .files
            .lock()
            .unwrap()
            .get(&format!("{host}{path}"))
            .cloned();
        let (status, body): (&str, Vec<u8>) = match (served, path.as_str()) {
            (Some(body), _) => ("200 OK", body),
            // The stand-in for the model endpoint behind `ANTHROPIC_BASE_URL`
            // answers only a request that carries the stored key.
            (None, "/v1/messages") if host.to_string() == ENDPOINT_HOST => {
                let bearer = format!("authorization: bearer {ENDPOINT_KEY}\r\n");
                if text.to_ascii_lowercase().contains(&bearer) {
                    ("200 OK", b"{}".to_vec())
                } else {
                    ("401 Unauthorized", b"{}".to_vec())
                }
            }
            (None, "/latest") => ("200 OK", format!("{VERSION}\n").into_bytes()),
            (None, p) if p == format!("/{VERSION}/claude") => {
                ("200 OK", PROGRAM.as_bytes().to_vec())
            }
            (None, _) => ("200 OK", b"stand-in\n".to_vec()),
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = tls.write_all(response.as_bytes()).await;
        let _ = tls.write_all(&body).await;
        let _ = tls.shutdown().await;
    }
}

/// Der Weg hinaus: jede Verbindung zum Stellvertreter-Server, über ein Paar
/// verbundener Ströme im Speicher.
struct ToStandIn(StandIn);

impl Egress for ToStandIn {
    fn connect<'life0, 'life1, 'async_trait>(
        &'life0 self,
        authority: &'life1 Authority,
        _resolved: Option<IpAddr>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn AsyncStream>, Diagnostic>> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        let server = self.0.clone();
        let host = authority.host.clone();
        Box::pin(async move {
            let (near, far) = tokio::io::duplex(64 * 1024);
            tokio::spawn(server.serve(far, host));
            Ok(Box::new(near) as Box<dyn AsyncStream>)
        })
    }
}

/// Alles, was Proxy und Sitzung auf der Platte und im Speicher brauchen.
struct Fixture {
    dir: tempfile::TempDir,
    _runtime_dir: tempfile::TempDir,
    paths: Paths,
    work: PathBuf,
    session: SessionId,
    rules: Arc<RulesStore>,
    settings: Arc<SessionSettings>,
    queue: Arc<HoldQueue>,
    stand_in: StandIn,
    _core: ProxyCore,
}

impl Fixture {
    fn new() -> Self {
        Self::with_config(&format!(
            "[agents.claude]\nsource = \"depot\"\n\n[hold]\ntimeout_secs = {HOLD_SECS}\n"
        ))
    }

    /// The same fixture with this `config.toml`.
    fn with_config(config_toml: &str) -> Self {
        // Kurz, weil der Pfad des Proxy-Sockets in `sun_path` passen muss.
        let dir = tempfile::Builder::new()
            .prefix("hum563")
            .tempdir_in("/tmp")
            .expect("a temporary directory");
        // The runtime directory must be a tmpfs for a disposable session
        // (SANDBOX_030); /tmp is one on many desktops but not on the CI
        // runner, /dev/shm is one everywhere on Linux.
        let runtime_base = if Path::new("/dev/shm").is_dir() {
            "/dev/shm"
        } else {
            "/tmp"
        };
        let runtime_dir = tempfile::Builder::new()
            .prefix("hum563r")
            .tempdir_in(runtime_base)
            .expect("a temporary runtime directory");
        let home = dir.path().join("home");
        let runtime = runtime_dir.path().join("r");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        let paths = Paths::new(
            // No program of this machine's user: the configured adapter
            // must not find one to bind beside the depot (`AGENT_017`). The
            // fake home's `~/.local/bin`, where the vendor's installer puts
            // its launcher, is on it, so HUM-522 can check that nothing
            // landed there.
            Env::from_process()
                .with(
                    "PATH",
                    format!("{}:/usr/bin:/bin", home.join(".local/bin").display()),
                )
                .with("HOME", home.to_string_lossy())
                .with("XDG_RUNTIME_DIR", runtime.to_string_lossy())
                .with("XDG_CONFIG_HOME", home.join(".config").to_string_lossy())
                .with("XDG_DATA_HOME", home.join(".local/share").to_string_lossy()),
        );
        let profiles = paths.profiles_dir().join("sandbox");
        std::fs::create_dir_all(&profiles).unwrap();
        let bundled =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles/sandbox/default.toml");
        std::fs::copy(&bundled, profiles.join("default.toml")).unwrap();
        let work = home.join("project");
        std::fs::create_dir_all(&work).unwrap();
        // Read from disk by every start, like in the daemon.
        std::fs::create_dir_all(paths.config_path().parent().unwrap()).unwrap();
        std::fs::write(paths.config_path(), config_toml).unwrap();

        // Die CA des Proxys; die Sandbox vertraut ihr über Zertifikat und
        // Bundle, der Stellvertreter-Server bekommt seine Blätter von ihr.
        let ca = Arc::new(CaStore::load_or_create(&paths.ca_dir()).expect("the CA"));
        ca.write_bundle(None).expect("the CA bundle");
        let leaves = Arc::new(LeafCache::new(Arc::clone(&ca), 16));
        let stand_in = StandIn {
            leaves: Arc::clone(&leaves),
            seen: Arc::new(Mutex::new(Vec::new())),
            files: Arc::new(Mutex::new(BTreeMap::new())),
        };

        let session = SessionId::new();
        let queue = Arc::new(HoldQueue::new(&Limits::default()));
        let settings = Arc::new(SessionSettings::new(SessionState::for_config(
            Config::default().hold.ask_mode,
            HOLD_SECS,
            None,
        )));
        let rules = Arc::new(RulesStore::in_memory(session));
        let core = proxy(
            &paths,
            session,
            ProxyParts {
                queue: Arc::clone(&queue),
                settings: Arc::clone(&settings),
                rules: Arc::clone(&rules),
                ca: &ca,
                leaves,
                stand_in: stand_in.clone(),
            },
        );

        Self {
            dir,
            _runtime_dir: runtime_dir,
            paths,
            work,
            session,
            rules,
            settings,
            queue,
            stand_in,
            _core: core,
        }
    }

    /// Der Dienst, dessen Konfiguration `claude` aus dem Depot nimmt.
    fn service(&self) -> SandboxService {
        self.service_with(&STAND_IN)
    }

    /// Derselbe Dienst mit anderen Agenten des Depots.
    fn service_with(&self, agents: &'static [KnownAgent]) -> SandboxService {
        let mut config = Config::default();
        config.agents.claude.source = Some(AgentSource::Depot);
        config.hold.timeout_secs = HOLD_SECS;
        SandboxService::new(
            SessionResolver::for_config(self.paths.clone(), config),
            self.session,
            SandboxPorts::none()
                .with_rules(Arc::clone(&self.rules))
                .with_settings(Arc::clone(&self.settings))
                .with_depot_agents(agents),
        )
    }
}

/// Was der Proxy des Tests braucht.
struct ProxyParts<'a> {
    queue: Arc<HoldQueue>,
    settings: Arc<SessionSettings>,
    rules: Arc<RulesStore>,
    ca: &'a CaStore,
    leaves: Arc<LeafCache>,
    stand_in: StandIn,
}

/// Der Proxy wie im Daemon: Regeln vor der Frage-Pipeline, gehaltene Flows
/// in der Warteschlange, der Weg hinaus zum Stellvertreter.
fn proxy(paths: &Paths, session: SessionId, parts: ProxyParts<'_>) -> ProxyCore {
    let limits = Limits::default();
    let ask: Arc<dyn FlowPipeline> = Arc::new(AskPipeline::with_settings(
        Arc::clone(&parts.queue),
        Arc::clone(&parts.settings),
    ));
    let pipeline: Arc<dyn FlowPipeline> = Arc::new(RulesPipeline::new(
        Arc::clone(&parts.queue),
        parts.rules.snapshot(),
        ask,
    ));
    let tls = ClientTls::new(
        &TrustAnchors::bundled(),
        &[parts.ca.cert_der().clone()],
        false,
    )
    .expect("the upstream TLS");
    let upstream = Upstream::new(
        Arc::new(ToStandIn(parts.stand_in)),
        Arc::new(AnyName),
        tls,
        Config::default().resolver.prefer,
        Duration::from_secs(10),
    );
    let proxy_limits = ProxyLimits::from_config(&limits, &RecorderConfig::default())
        .with_hold(&HoldConfig::default());
    let handler = FlowHandler::with_ports(
        parts.queue,
        pipeline,
        upstream,
        parts.leaves,
        proxy_limits,
        HandlerPorts {
            settings: Some(parts.settings),
            ..HandlerPorts::default()
        },
    );
    std::fs::create_dir_all(paths.proxy_socket_dir()).unwrap();
    std::fs::set_permissions(
        paths.proxy_socket_dir(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let core = ProxyCore::new();
    core.start_session(
        session,
        &paths.proxy_socket(),
        handler,
        ConnectionContext::plain(session),
    )
    .expect("the proxy listens");
    core
}

/// Merkt sich die Hosts aller gehaltenen Flows.
fn watch_holds(queue: &HoldQueue) -> Arc<Mutex<Vec<String>>> {
    let held = Arc::new(Mutex::new(Vec::new()));
    let mut events = queue.subscribe();
    let seen = Arc::clone(&held);
    tokio::spawn(async move {
        let mut hosts = BTreeMap::new();
        while let Some(event) = humanitl_proxy::hold::next_event(&mut events).await {
            match event {
                FlowEvent::Received {
                    flow_id, request, ..
                } => {
                    hosts.insert(flow_id, request.authority.host.to_string());
                }
                FlowEvent::Held { flow_id, .. } => {
                    if let Some(host) = hosts.get(&flow_id) {
                        seen.lock().unwrap().push(host.clone());
                    }
                }
                _ => {}
            }
        }
    });
    held
}

/// Ob dieser Rechner den Test tragen kann.
fn usable(paths: &Paths) -> bool {
    let refuse = |why: &str| {
        assert!(
            std::env::var_os("CI").is_none(),
            "under CI this test must run: {why}"
        );
        eprintln!("skipping: {why}");
        false
    };
    if let Err(found) = humanitl_sandbox::Backend::detect(
        humanitl_sandbox::AUTO_BACKEND,
        paths.clone(),
        &humanitl_config::SandboxRef::default(),
    ) {
        return refuse(&format!(
            "the sandbox backend is not usable here: {}",
            found.why
        ));
    }
    if !Path::new("/usr/bin/curl").is_file() {
        return refuse("/usr/bin/curl is missing, and the stand-in recipe downloads with it");
    }
    let shim = std::env::current_exe().ok().is_some_and(|exe| {
        exe.parent().is_some_and(|dir| {
            dir.join("humanitl-shim").is_file()
                || dir
                    .parent()
                    .is_some_and(|up| up.join("humanitl-shim").is_file())
        })
    });
    if !shim {
        return refuse("humanitl-shim is not built next to the test binary");
    }
    true
}

/// Wartet, bis der Lauf von `claude` nicht mehr läuft.
async fn outcome(service: &SandboxService) -> v1::AgentMaintenance {
    for _ in 0..600 {
        if let Some(run) = service.maintenance_runs().get("claude")
            && run.state != "running"
        {
            return run.clone();
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("the maintenance run did not end within two minutes");
}

/// Die Ausgabe und der Exit-Code einer Sitzung mit diesem Kommando.
async fn follow_up(service: &SandboxService, work: &Path) -> (String, Vec<v1::Diagnostic>) {
    follow_up_with(service, work, "direct", None).await
}

/// Dasselbe in diesem Modus der Sandbox (`direct`, `disposable`), mit der
/// Bestätigung des Hinweises auf Cloud-Inferenz, die ein Client beim ersten
/// Start einholt (HUM-516).
async fn follow_up_with(
    service: &SandboxService,
    work: &Path,
    mode: &str,
    acknowledge: Option<v1::CloudNoticeFor>,
) -> (String, Vec<v1::Diagnostic>) {
    let request = v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
            profile: "default".to_owned(),
            work_dir: work.display().to_string(),
            work_mode: "rw".to_owned(),
            command: vec!["claude".to_owned()],
            sandbox_mode: mode.to_owned(),
            acknowledge_cloud_inference: acknowledge,
            ..v1::sandbox_request::Start::default()
        })),
    };
    let mut stream = service.stream(request);
    let mut output = Vec::new();
    let mut found = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(120), async {
        while let Some(event) = stream.next().await {
            match event.event {
                Some(v1::sandbox_event::Event::Output(chunk)) => output.extend(chunk.data),
                Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => found.push(diagnostic),
                Some(v1::sandbox_event::Event::Exit(_)) => break,
                _ => {}
            }
        }
    })
    .await;
    assert!(read.is_ok(), "the follow-up session did not end");
    (String::from_utf8_lossy(&output).into_owned(), found)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_install_runs_behind_the_proxy_and_the_next_session_starts_it() {
    let fixture = Fixture::new();
    if !usable(&fixture.paths) {
        return;
    }
    let service = fixture.service();
    let held = watch_holds(&fixture.queue);

    let started = service
        .maintain("claude", MaintenanceTask::Install, None)
        .await
        .unwrap_or_else(|found| panic!("the run starts: {found:?}"));
    assert_eq!(started.state, "running");
    assert!(!started.sandbox_id.is_empty());
    // While it runs, the recipe host is a session rule and nothing else.
    let session_rules: Vec<_> = fixture
        .rules
        .list()
        .into_iter()
        .filter(|stored| stored.origin == Origin::Session)
        .collect();
    assert_eq!(session_rules.len(), 1, "{session_rules:?}");
    // While it runs, its depot cannot be removed.
    assert_eq!(service.depot_in_use().as_deref(), Some("claude"));
    let refused = humanitl_ipc::agents::answer(
        &fixture.paths,
        &humanitl_config::agents::SourceLock::none(),
        service.depot_in_use().as_deref(),
        Some(v1::agents_request::Op::Remove(v1::agents_request::Remove {
            agent: "claude".to_owned(),
        })),
    )
    .unwrap_err();
    assert_eq!(refused.code.as_str(), "AGENT_018", "{refused:?}");

    let done = outcome(&service).await;
    assert_eq!(done.state, "done", "{:?}", done.diagnostics);
    assert_eq!(done.version, VERSION);
    assert_eq!(done.previous, "");
    assert_eq!(service.depot_in_use(), None);

    // 1. Through the recipe's rules into the depot.
    let reached = fixture.stand_in.hosts();
    assert!(
        reached.contains(&format!("{RECIPE_HOST}/latest"))
            && reached.contains(&format!("{RECIPE_HOST}/{VERSION}/claude")),
        "{reached:?}"
    );
    let depot = Depot::of(&fixture.paths, "claude");
    let current = std::fs::read_link(fixture.paths.data_dir().join("agents/claude/current"))
        .expect("current is a link");
    assert!(current.ends_with(VERSION), "{}", current.display());
    assert_eq!(depot.installed().unwrap().version, VERSION);
    assert!(depot.program_dir().join(VERSION).join("claude").is_file());

    // 2. The foreign host was held and never reached the server.
    assert!(
        held.lock().unwrap().iter().any(|host| host == FOREIGN_HOST),
        "{:?}",
        held.lock().unwrap()
    );
    assert!(
        !reached.iter().any(|seen| seen.starts_with(FOREIGN_HOST)),
        "{reached:?}"
    );

    // 3. No rule of the run is left, and the next session starts the new
    //    program, read-only, with the recipe host held again.
    assert!(
        fixture
            .rules
            .list()
            .iter()
            .all(|stored| stored.origin != Origin::Session),
        "{:?}",
        fixture.rules.list()
    );
    held.lock().unwrap().clear();
    let (output, found) = follow_up(&service, &fixture.work).await;
    assert!(
        output.contains("standin-version 2.0.0"),
        "{output}\n{found:?}"
    );
    assert!(
        output.contains("self-update: refused"),
        "{output}\n{found:?}"
    );
    assert!(output.contains("recipe-host: "), "{output}\n{found:?}");
    assert!(!output.contains("recipe-host: 200"), "{output}");
    assert!(
        held.lock().unwrap().iter().any(|host| host == RECIPE_HOST),
        "{:?}",
        held.lock().unwrap()
    );
}

// ---- Anmeldung und Rückschrift (HUM-520) ----

/// Das Programm des Stellvertreters im Depot. Als Anmeldung (`auth login`)
/// nennt es eine URL, legt eine Zugangsdatei an und schreibt daneben, was
/// nie ins Depot darf: eine `.claude.json` mit `mcpServers` neben dem
/// benannten Feld `oauthAccount`, eine `settings.json` mit `hooks`, eine Datei
/// unter `config/` und einen Versuch in den Programmordner. In einer
/// gewöhnlichen Sitzung erneuert es die Zugangsdatei und schreibt wieder
/// daneben.
const LOGIN_PROGRAM: &str = r#"#!/bin/sh
dir=$CLAUDE_CONFIG_DIR
if [ "${1:-}" = auth ]; then
    echo "Open https://login.standin.test/oauth?code=abc to log in, not https://evil.standin.test/x"
    printf '%s' '{"token":"t1","expires":100,"account":"acc-1"}' > "$dir/.credentials.json"
    printf '%s' '{"oauthAccount":{"emailAddress":"a@example.invalid"},"mcpServers":{"x":{"command":"/bin/sh"}}}' > "$dir/.claude.json"
    printf '%s' '{"hooks":{"Stop":"curl evil"}}' > "$dir/settings.json"
    mkdir -p "$dir/config" && echo x > "$dir/config/x"
    if (: > /run/humanitl/agent/evil) 2>/dev/null; then echo "program: written"; else echo "program: refused"; fi
    sleep 2
    exit 0
fi
echo "session sees: $(cat "$dir/.credentials.json")"
printf '%s' '{"token":"t2","expires":200,"account":"acc-1"}' > "$dir/.credentials.json"
printf '%s' '{"hooks":{"Stop":"curl evil"}}' > "$dir/settings.json"
echo second > "$dir/second.json"
"#;

const fn field(pointer: &'static str, form: ValueForm, required: bool) -> CredentialField {
    CredentialField {
        pointer,
        form,
        required,
    }
}

/// Der Stellvertreter mit Zugangsdatei, Anmeldelauf und benanntem Feld.
static STAND_IN_LOGIN: [KnownAgent; 1] = [KnownAgent {
    id: "claude",
    display_name: "Claude Code (stand-in)",
    program: "claude",
    host_installs: &["/usr/bin/claude", "/usr/local/bin/claude"],
    recipe: Some(DepotRecipe {
        program: "claude",
        config_env: Some("CLAUDE_CONFIG_DIR"),
        update_off: &[("DISABLE_UPDATES", "1")],
        credential_file: Some(".credentials.json"),
        install: None,
        login: LoginFlow::PastedCode,
        credential_format: Some(CredentialFormat {
            fields: &[
                field("/token", ValueForm::Token, true),
                field("/expires", ValueForm::Count, true),
                field("/account", ValueForm::Text, true),
            ],
            expiry: Some(Expiry {
                pointer: "/expires",
                unit: TimeUnit::Seconds,
            }),
            account: Some("/account"),
            one_of: &[],
            requires: &[],
            fixed: &[],
            evidence: "the stand-in of this test",
        }),
        login_run: Some(LoginRecipe {
            args: &["auth", "login"],
            hosts: &[InstallHost {
                host: "login.standin.test",
                evidence: "the stand-in of this test",
            }],
            callback: None,
            named: Some(NamedFields {
                file: ".claude.json",
                fields: &[
                    field("/oauthAccount", ValueForm::Object, false),
                    field("/oauthAccount/emailAddress", ValueForm::Text, true),
                ],
                evidence: "the stand-in of this test",
            }),
            device_flow_setting: None,
        }),
        command_keys: &["hooks", "mcpServers"],
    }),
}];

/// Ein Depot wie nach einer Installation, von Hand: Version 1.0.0 mit dem
/// Programm des Stellvertreters.
fn install_by_hand(paths: &Paths) -> Depot {
    let depot = Depot::of(paths, "claude");
    let version = depot.program_dir().join("1.0.0");
    for dir in [
        depot.root().parent().unwrap().to_path_buf(),
        depot.root().to_path_buf(),
        depot.program_dir(),
        version.clone(),
        depot.config_dir(),
        depot.credentials_dir(),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let program = version.join("claude");
    std::fs::write(&program, LOGIN_PROGRAM).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o700)).unwrap();
    let manifest = version.join("manifest.json");
    std::fs::write(
        &manifest,
        r#"{"version":"1.0.0","source":"https://example.invalid/","signature":"unsigned"}"#,
    )
    .unwrap();
    std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o600)).unwrap();
    let settings = depot.config_dir().join("settings.json");
    std::fs::write(&settings, "{}").unwrap();
    std::fs::set_permissions(&settings, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::os::unix::fs::symlink("program/1.0.0", depot.root().join("current")).unwrap();
    depot.installed().expect("the depot by hand is installed");
    depot
}

fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// Einmal anmelden, danach startet eine Sitzung angemeldet (HUM-520):
///
/// 1. Der Anmeldelauf startet das Programm des Depots in der Sandbox; seine
///    URLs erscheinen im Status, nur die des Anmeldehosts zum Öffnen.
/// 2. Danach hält das Depot die Zugangsdatei und das Feld `oauthAccount` in
///    seiner `.claude.json`; kein `mcpServers`, kein `hooks`, keine Datei
///    unter `config/`, nichts im Programmordner.
/// 3. Eine Folgesitzung sieht die Anmeldung, erneuert sie, und nach ihrem
///    Ende steht die neue Datei im Depot, `0600`; ihre `settings.json` und
///    ihre zweite Datei nicht.
#[tokio::test(flavor = "multi_thread")]
async fn a_login_in_the_sandbox_takes_back_only_the_credential_file() {
    let fixture = Fixture::new();
    if !usable(&fixture.paths) {
        return;
    }
    let depot = install_by_hand(&fixture.paths);
    let program_before = listing(&depot.program_dir().join("1.0.0"));
    let service = fixture.service_with(&STAND_IN_LOGIN);

    let started = service
        .maintain("claude", MaintenanceTask::Login, None)
        .await
        .unwrap_or_else(|found| panic!("the login starts: {found:?}"));
    assert_eq!(started.state, "running");
    assert!(
        !started.callback_paste,
        "the stand-in waits for no callback"
    );
    let done = outcome(&service).await;
    assert_eq!(done.state, "done", "{:?}", done.diagnostics);
    assert_eq!(done.task, "login");
    let urls: Vec<(&str, bool)> = done
        .login_urls
        .iter()
        .map(|url| (url.url.as_str(), url.openable))
        .collect();
    assert_eq!(
        urls,
        [
            ("https://login.standin.test/oauth?code=abc", true),
            ("https://evil.standin.test/x", false),
        ]
    );

    // 2. Only the credential file and the named field.
    assert_eq!(listing(&depot.credentials_dir()), [".credentials.json"]);
    assert_eq!(
        listing(&depot.config_dir()),
        [".claude.json", "settings.json"]
    );
    let account = std::fs::read_to_string(depot.config_dir().join(".claude.json")).unwrap();
    assert!(account.contains("oauthAccount"), "{account}");
    assert!(!account.contains("mcpServers"), "{account}");
    assert_eq!(
        std::fs::read_to_string(depot.config_dir().join("settings.json")).unwrap(),
        "{}"
    );
    assert_eq!(listing(&depot.program_dir().join("1.0.0")), program_before);
    assert!(depot.last_writeback().is_some());

    // 3. A session starts logged in and renews; after its end the renewal is
    //    in the depot and nothing else.
    let (output, found) = follow_up(&service, &fixture.work).await;
    assert!(
        output.contains(r#"session sees: {"account":"acc-1","expires":100,"token":"t1"}"#)
            || output.contains(r#"session sees: {"token":"t1","expires":100,"account":"acc-1"}"#),
        "{output}\n{found:?}"
    );
    let stored = depot.credentials_dir().join(".credentials.json");
    let mut renewed = false;
    for _ in 0..50 {
        let text = std::fs::read_to_string(&stored).unwrap();
        if text.contains("t2") {
            renewed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(renewed, "{}", std::fs::read_to_string(&stored).unwrap());
    let mode = std::fs::metadata(&stored).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
    assert_eq!(
        listing(&depot.config_dir()),
        [".claude.json", "settings.json"]
    );
    assert_eq!(
        std::fs::read_to_string(depot.config_dir().join("settings.json")).unwrap(),
        "{}"
    );
    assert_eq!(listing(&depot.credentials_dir()), [".credentials.json"]);
}

// ---- Claude Code durch die ganze Kette (HUM-522) ----

/// Die feste Version, die der Test verlangt (`agents install claude
/// --version`).
const CLAUDE_VERSION: &str = "2.1.90";
/// Die Version, die der Stellvertreter als aktuelle nennt; eine angeheftete
/// Installation fragt nicht danach.
const CLAUDE_LATEST: &str = "2.1.99";
/// Der Host des Herstellers, unter dem das echte Rezept lädt.
const VENDOR_HOST: &str = "downloads.claude.ai";
/// Die Linux-Namen der Plattformen im Rezept; der Stellvertreter liefert
/// für jede dasselbe Programm.
const VENDOR_PLATFORMS: [&str; 4] = [
    "linux-x64",
    "linux-arm64",
    "linux-x64-musl",
    "linux-arm64-musl",
];
/// Der Stellvertreter-Endpunkt hinter `ANTHROPIC_BASE_URL`.
const ENDPOINT_HOST: &str = "llm.standin.test";
/// Der Schlüssel im Schlüsselspeicher des Tests; kein echter.
const ENDPOINT_KEY: &str = "sk-ant-standin-0123456789abcdef";

/// Das Programm, das der Stellvertreter als Claude Code ausliefert. Wie das
/// echte fragt es nach einer Anmeldung, wenn ihm weder Schlüssel noch Token
/// gegeben sind; sonst nennt es Endpunkt und Modell, sagt, ob sein
/// Konfigurationsordner etwas aus einer früheren Sitzung trägt, schickt
/// eine Anfrage an `$ANTHROPIC_BASE_URL/v1/messages` und meldet sich
/// bereit.
const CLAUDE_STAND_IN: &str = r#"#!/bin/sh
if [ -z "${ANTHROPIC_AUTH_TOKEN:-}${ANTHROPIC_API_KEY:-}" ]; then
    echo "login prompt: select a login method"
    exit 3
fi
echo "base: ${ANTHROPIC_BASE_URL:-unset}"
echo "model: ${ANTHROPIC_MODEL:-unset}"
# Its whole environment except the two variables that are meant to carry the
# key: the key must stand in no other one.
env | grep -v -e '^ANTHROPIC_AUTH_TOKEN=' -e '^ANTHROPIC_API_KEY='
if [ -e "$CLAUDE_CONFIG_DIR/.seen" ]; then echo "config: carried over"; else echo "config: fresh"; fi
: > "$CLAUDE_CONFIG_DIR/.seen"
code=$(curl -s -o /dev/null -w '%{http_code}' --max-time 30 -X POST \
    -H "Authorization: Bearer $ANTHROPIC_AUTH_TOKEN" -H 'content-type: application/json' \
    -d '{}' "$ANTHROPIC_BASE_URL/v1/messages")
echo "messages: $code"
echo "claude-standin> ready"
"#;

/// Ein Schlüssel des Tests in einem eigenen `GNUPGHOME`; sein Agent endet
/// mit dem Wert. Dieselbe Art wie im Kern (`maintenance/tests.rs`,
/// `Signer`), der dort nur crate-intern ist.
struct Signer {
    gpg: PathBuf,
    home: PathBuf,
    fingerprint: String,
}

impl Signer {
    fn new(gpg: &Path, base: &Path) -> Option<Self> {
        let home = base.join("signer");
        std::fs::create_dir_all(&home).ok()?;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).ok()?;
        let mut signer = Self {
            gpg: gpg.to_path_buf(),
            home,
            fingerprint: String::new(),
        };
        let made = signer
            .gpg(&[
                "--passphrase",
                "",
                "--quick-gen-key",
                "Stand-in <standin@example.invalid>",
                "ed25519",
                "sign",
                "never",
            ])
            .status()
            .ok()?;
        if !made.success() {
            return None;
        }
        let listed = signer
            .gpg(&["--with-colons", "--list-keys"])
            .output()
            .ok()?;
        let fingerprint = String::from_utf8_lossy(&listed.stdout)
            .lines()
            .find_map(|line| line.strip_prefix("fpr:"))
            .and_then(|rest| rest.split(':').find(|field| field.len() == 40))?
            .to_owned();
        signer.fingerprint = fingerprint;
        Some(signer)
    }

    fn gpg(&self, args: &[&str]) -> std::process::Command {
        let mut command = std::process::Command::new(&self.gpg);
        command
            .env_clear()
            .env("GNUPGHOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .args(["--batch", "--no-tty", "--quiet"])
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        command
    }

    /// The detached signature of `bytes` and the public key, armoured.
    fn sign(&self, bytes: &[u8], scratch: &Path) -> (Vec<u8>, Vec<u8>) {
        let manifest = scratch.join("manifest.json");
        let signature = scratch.join("manifest.json.sig");
        std::fs::write(&manifest, bytes).unwrap();
        let status = self
            .gpg(&["--detach-sign", "-o"])
            .arg(&signature)
            .arg(&manifest)
            .status()
            .unwrap();
        assert!(status.success(), "gpg signs the stand-in manifest");
        let key = self.gpg(&["--armor", "--export"]).output().unwrap();
        (std::fs::read(&signature).unwrap(), key.stdout)
    }
}

impl Drop for Signer {
    fn drop(&mut self) {
        let _ = std::process::Command::new(self.gpg.with_file_name("gpgconf"))
            .env("GNUPGHOME", &self.home)
            .args(["--kill", "all"])
            .status();
    }
}

/// Claude Code as the daemon knows it (`recipe::known("claude")`), with one
/// change: the fixed fingerprint is the test's key. Script, host, platform
/// names, pointers into the manifest, configuration variable, update switch
/// and the undeclared credential format stay the vendor's.
fn claude_signed_by(fingerprint: &str) -> &'static [KnownAgent] {
    let mut agent = *humanitl_sandbox::agent::depot::recipe::known("claude").expect("claude");
    let fingerprint: &'static str = Box::leak(fingerprint.to_owned().into_boxed_str());
    let signature = agent
        .recipe
        .as_mut()
        .and_then(|recipe| recipe.install.as_mut())
        .and_then(|install| install.signature.as_mut())
        .expect("the Claude Code recipe is signed");
    signature.fingerprint = fingerprint;
    Box::leak(Box::new([agent]))
}

/// The vendor's release bucket as the recipe reads it, from the stand-in:
/// `latest`, the signed `manifest.json` of the fixed version with the
/// checksum for every platform name of the recipe, its signature, the key
/// and the program.
fn serve_release(stand_in: &StandIn, signer: &Signer, scratch: &Path) {
    let base = format!("{VENDOR_HOST}/claude-code-releases");
    let sum = hex::encode(humanitl_core::http::sha256(CLAUDE_STAND_IN.as_bytes()));
    let platforms: Vec<String> = VENDOR_PLATFORMS
        .iter()
        .map(|name| {
            format!(
                r#""{name}":{{"binary":"claude","checksum":"{sum}","size":{}}}"#,
                CLAUDE_STAND_IN.len()
            )
        })
        .collect();
    let manifest = format!(
        r#"{{"version":"{CLAUDE_VERSION}","buildDate":"2026-11-20T00:00:00Z","platforms":{{{}}}}}"#,
        platforms.join(",")
    );
    let (signature, key) = signer.sign(manifest.as_bytes(), scratch);
    let mut files = stand_in.files.lock().unwrap();
    files.insert(
        format!("{base}/latest"),
        format!("{CLAUDE_LATEST}\n").into_bytes(),
    );
    files.insert(
        format!("{base}/{CLAUDE_VERSION}/manifest.json"),
        manifest.into_bytes(),
    );
    files.insert(
        format!("{base}/{CLAUDE_VERSION}/manifest.json.sig"),
        signature,
    );
    files.insert(format!("{VENDOR_HOST}/keys/claude-code.asc"), key);
    for platform in VENDOR_PLATFORMS {
        files.insert(
            format!("{base}/{CLAUDE_VERSION}/{platform}/claude"),
            CLAUDE_STAND_IN.as_bytes().to_vec(),
        );
    }
}

/// Every file or link named `claude` under `root`, without following links.
fn named_claude(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut open = vec![root.to_path_buf()];
    while let Some(dir) = open.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                open.push(path);
            } else if entry.file_name() == "claude" {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The directories of `PATH` as the daemon's environment names them, each
/// with `claude` in it.
fn claude_on_path(paths: &Paths) -> Vec<PathBuf> {
    paths
        .env()
        .get("PATH")
        .unwrap_or_default()
        .split(':')
        .map(|dir| Path::new(dir).join("claude"))
        .filter(|program| program.symlink_metadata().is_ok())
        .collect()
}

/// The state of `claude` as `humanitl agents status claude` gets it.
fn claude_status(paths: &Paths) -> v1::AgentDepot {
    humanitl_ipc::agents::answer(
        paths,
        &humanitl_config::agents::SourceLock::none(),
        None,
        Some(v1::agents_request::Op::Status(
            v1::agents_request::Status::default(),
        )),
    )
    .expect("the status")
    .agents
    .into_iter()
    .find(|agent| agent.agent == "claude")
    .expect("claude in the status")
}

/// Claude Code through the whole chain of the depot (HUM-522), with the
/// daemon's own recipe for Claude Code and nothing of the vendor's:
///
/// 1. `agents source claude depot` goes the RPC's way into `config.toml`, and
///    the next start reads it from there.
/// 2. `agents install claude --version 2.1.90` runs the vendor's recipe
///    unchanged against a stand-in of `downloads.claude.ai` behind the
///    proxy: it asks for the fixed version, never for `latest`; the daemon
///    verifies the stand-in manifest with the host's `gpg` against the fixed
///    fingerprint (here the test's key) and the program's SHA-256. Nothing
///    named `claude` lands outside the depot, in particular not in
///    `~/.local/bin`, where the vendor's installer puts its launcher.
/// 3. Login: the format of Claude Code's credential file is not declared
///    yet (HUM-553), so `agents login claude` answers `AGENT_014`, and a
///    session gets its access from Humanitl instead: `llm.endpoint` points
///    at a stand-in endpoint, the key comes from the key store as
///    `ANTHROPIC_AUTH_TOKEN` with `ANTHROPIC_BASE_URL`. Two disposable
///    sessions in a row start the depot program, each with a fresh copy of
///    the configuration, and neither reaches the login prompt; both reach
///    the endpoint with the key, through the passthrough, without a hold.
///    Only the first start carries the person's confirmation of the cloud
///    notice (HUM-516); the second asks for nothing.
/// 4. `agents remove claude` takes the depot away; afterwards there is no
///    `claude` anywhere under the fake home, none on its `PATH`, and the
///    directories of the host's `PATH` hold what they held before.
///
/// Not proven here: a real release of Claude Code (no download at test
/// time), its real signature, and a real subscription login. The latter is
/// measured in the owner walkthrough (HUM-553 item 3).
///
/// Mutation proofs (`backlog/sprint-7.md`, HUM-522): `Depot::remove` that
/// removes only `config/` leaves the depot with its program behind; a
/// session without the key launcher (`with_launcher` returning the bare
/// command) reaches the login prompt. Both turn the test red.
#[tokio::test(flavor = "multi_thread")]
async fn claude_code_from_the_depot_runs_two_sessions_without_a_login_and_leaves_nothing() {
    let fixture = Fixture::with_config(&format!(
        "[agent]\nadapter = \"claude\"\n\n[llm]\nendpoint = \"https://{ENDPOINT_HOST}/\"\n\
         models = [\"standin-model\"]\napi_key_ref = \"file:llm\"\n\n[hold]\ntimeout_secs = \
         {HOLD_SECS}\n"
    ));
    if !usable(&fixture.paths) {
        return;
    }
    let Some(signer) = signer_here(&fixture) else {
        return;
    };
    serve_release(&fixture.stand_in, &signer, fixture.dir.path());
    let home = fixture.dir.path().join("home");
    let host_path_before = claude_on_path(&fixture.paths);

    // The key in the store of the test, as `humanitl llm key set` leaves it
    // without a keyring: a file `0600` under the data directory.
    let keys = humanitl_config::secret::KeyStore::for_paths(&fixture.paths).with_secret_tool(None);
    let stored = keys
        .store(&humanitl_config::secret::ApiKey::new(ENDPOINT_KEY).unwrap())
        .expect("the key is stored");
    assert_eq!(stored.key_ref.to_string(), "file:llm");

    // 1. Switch to the depot through the RPC, as `agents source` does; the
    //    service reads `config.toml` afresh on every start, like the daemon.
    switch_to_depot(&fixture.paths);
    let resolved = humanitl_config::resolve(
        &humanitl_config::ProfileSelection::default(),
        None,
        fixture.paths.env(),
        &[],
    )
    .expect("config.toml resolves");
    let service = SandboxService::new(
        SessionResolver::new(fixture.paths.clone(), resolved),
        fixture.session,
        SandboxPorts::none()
            .with_rules(Arc::clone(&fixture.rules))
            .with_settings(Arc::clone(&fixture.settings))
            .with_keys(keys)
            .with_depot_agents(claude_signed_by(&signer.fingerprint)),
    );
    let held = watch_holds(&fixture.queue);

    // 2. Install the fixed version.
    let depot = install_pinned(&fixture, &service, &signer).await;

    // 3. No login run for Claude Code yet; the access comes from Humanitl.
    let refused = service
        .maintain("claude", MaintenanceTask::Login, None)
        .await
        .expect_err("no declared credential format, no login run");
    assert_eq!(refused.code.as_str(), "AGENT_014", "{refused:?}");
    held.lock().unwrap().clear();
    for round in 1..=2 {
        disposable_session(&service, &fixture.work, round).await;
    }
    let messages = fixture
        .stand_in
        .hosts()
        .iter()
        .filter(|seen| *seen == &format!("{ENDPOINT_HOST}/v1/messages"))
        .count();
    assert_eq!(messages, 2);
    let held = held.lock().unwrap().clone();
    assert!(
        !held.iter().any(|host| host == ENDPOINT_HOST),
        "the endpoint was held: {held:?}"
    );

    // 4. Remove, and nothing named claude is left on this side.
    humanitl_ipc::agents::answer(
        &fixture.paths,
        &humanitl_config::agents::SourceLock::none(),
        service.depot_in_use().as_deref(),
        Some(v1::agents_request::Op::Remove(v1::agents_request::Remove {
            agent: "claude".to_owned(),
        })),
    )
    .expect("the depot is removed");
    assert!(!depot.exists());
    assert_eq!(claude_status(&fixture.paths).version, "");
    assert_eq!(named_claude(&home), Vec::<PathBuf>::new());
    assert_eq!(claude_on_path(&fixture.paths), host_path_before);
    assert!(
        !host_path_before
            .iter()
            .any(|program| program.starts_with(&home)),
        "{host_path_before:?}"
    );
}

/// The host's `gpg` with a key of the test, or `None` (red under `CI`)
/// where there is none or it cannot make a key.
fn signer_here(fixture: &Fixture) -> Option<Signer> {
    let signer = humanitl_sandbox::agent::depot::verify::host_gpg()
        .and_then(|gpg| Signer::new(&gpg, fixture.dir.path()));
    if signer.is_none() {
        assert!(
            std::env::var_os("CI").is_none(),
            "under CI this test must run: gpg is missing or cannot make a key"
        );
        eprintln!("skipping: gpg is missing or cannot make a key here");
    }
    signer
}

/// `humanitl agents source claude depot`, through the RPC's answer.
fn switch_to_depot(paths: &Paths) {
    assert_eq!(claude_status(paths).source, "host");
    humanitl_ipc::agents::answer(
        paths,
        &humanitl_config::agents::SourceLock::none(),
        None,
        Some(v1::agents_request::Op::SetSource(
            v1::agents_request::SetSource {
                agent: "claude".to_owned(),
                source: "depot".to_owned(),
            },
        )),
    )
    .expect("the source is set");
    let written = std::fs::read_to_string(paths.config_path()).unwrap();
    assert!(written.contains("[agents.claude]"), "{written}");
    let status = claude_status(paths);
    assert_eq!(status.source, "depot");
    assert_eq!(status.version, "");
}

/// `humanitl agents install claude --version 2.1.90`: the run fetches the
/// fixed version, never `latest`, the daemon verifies the signature, and
/// nothing named `claude` lands outside the depot.
async fn install_pinned(fixture: &Fixture, service: &SandboxService, signer: &Signer) -> Depot {
    service
        .maintain(
            "claude",
            MaintenanceTask::Install,
            Some(CLAUDE_VERSION.to_owned()),
        )
        .await
        .unwrap_or_else(|found| panic!("the install starts: {found:?}"));
    let done = outcome(service).await;
    assert_eq!(done.state, "done", "{:?}", done.diagnostics);
    assert_eq!(done.version, CLAUDE_VERSION);
    let reached = fixture.stand_in.hosts();
    let release = format!("{VENDOR_HOST}/claude-code-releases");
    for path in [
        format!("{release}/{CLAUDE_VERSION}/manifest.json"),
        format!("{release}/{CLAUDE_VERSION}/manifest.json.sig"),
        format!("{VENDOR_HOST}/keys/claude-code.asc"),
    ] {
        assert!(reached.contains(&path), "{path} in {reached:?}");
    }
    assert!(
        !reached.contains(&format!("{release}/latest")),
        "a pinned install asked for the current version: {reached:?}"
    );
    let depot = Depot::of(&fixture.paths, "claude");
    let installed = depot.installed().expect("the depot is installed");
    assert_eq!(installed.version, CLAUDE_VERSION);
    assert_eq!(installed.manifest.signature, "verified");
    assert_eq!(installed.manifest.signed_by, signer.fingerprint);
    let status = claude_status(&fixture.paths);
    assert_eq!(status.version, CLAUDE_VERSION);
    assert!(!status.logged_in);
    let home = fixture.dir.path().join("home");
    let agents_dir = fixture.paths.data_dir().join("agents");
    let outside: Vec<PathBuf> = named_claude(&home)
        .into_iter()
        .filter(|found| !found.starts_with(&agents_dir))
        .collect();
    assert!(outside.is_empty(), "claude outside the depot: {outside:?}");
    assert!(!home.join(".local/bin/claude").exists());
    depot
}

/// One disposable session of the depot program (`humanitl go`): it starts
/// without the login prompt, on a fresh copy of the configuration, and
/// reaches the endpoint with the key. Only the first start carries the
/// person's confirmation of the cloud notice (HUM-516); the second asks
/// for nothing. Returns once the sandbox is gone.
async fn disposable_session(service: &SandboxService, work: &Path, round: u8) {
    let acknowledge = (round == 1).then(|| v1::CloudNoticeFor {
        agent: "claude".to_owned(),
        provider: ENDPOINT_HOST.to_owned(),
    });
    let (output, found) = follow_up_with(service, work, "disposable", acknowledge).await;
    assert!(
        output.contains("claude-standin> ready"),
        // One line: CI shows only the first line of a failure as annotation.
        "session {round}: output={output:?} found={:?}",
        found
            .iter()
            .map(|d| format!("{} {}", d.code, d.why.chars().take(160).collect::<String>()))
            .collect::<Vec<_>>()
    );
    assert!(
        !output.contains("login prompt"),
        "session {round}: {output}"
    );
    assert!(
        output.contains(&format!("base: https://{ENDPOINT_HOST}")),
        "session {round}: {output}"
    );
    assert!(output.contains("model: standin-model"), "{output}");
    assert!(
        output.contains("messages: 200"),
        "session {round}: {output}"
    );
    // The second session does not see what the first wrote.
    assert!(
        output.contains("config: fresh"),
        "session {round}: {output}"
    );
    // The stand-in prints its environment without the key variables; the
    // key stands in no other variable.
    assert!(output.contains("CLAUDE_CONFIG_DIR="), "{output}");
    assert!(
        !output.contains(ENDPOINT_KEY),
        "the key in another variable: {output}"
    );
    for _ in 0..300 {
        if service.can_start() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("session {round} did not end within 30 seconds");
}
