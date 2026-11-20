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
//! Braucht das Sandbox-Backend des Systems, unprivilegierte Nutzer-Namensräume, `curl` im `/usr` des
//! Hosts und den gebauten Shim; fehlt etwas, sagt er es und endet grün,
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
        config_env: "CLAUDE_CONFIG_DIR",
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
        let body: Vec<u8> = match path.as_str() {
            "/latest" => format!("{VERSION}\n").into_bytes(),
            p if p == format!("/{VERSION}/claude") => PROGRAM.as_bytes().to_vec(),
            _ => b"stand-in\n".to_vec(),
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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
    _dir: tempfile::TempDir,
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
        // Kurz, weil der Pfad des Proxy-Sockets in `sun_path` passen muss.
        let dir = tempfile::Builder::new()
            .prefix("hum563")
            .tempdir_in("/tmp")
            .expect("a temporary directory");
        let runtime_dir = tempfile::Builder::new()
            .prefix("hum563r")
            .tempdir_in("/tmp")
            .expect("a temporary runtime directory");
        let home = dir.path().join("home");
        let runtime = runtime_dir.path().join("r");
        std::fs::create_dir_all(&home).unwrap();
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o700)).unwrap();
        let paths = Paths::new(
            // No program of this machine's user: the configured adapter
            // must not find one to bind beside the depot (`AGENT_017`).
            Env::from_process()
                .with("PATH", "/usr/bin:/bin")
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
        std::fs::write(
            paths.config_path(),
            format!("[agents.claude]\nsource = \"depot\"\n\n[hold]\ntimeout_secs = {HOLD_SECS}\n"),
        )
        .unwrap();

        // Die CA des Proxys; die Sandbox vertraut ihr über Zertifikat und
        // Bundle, der Stellvertreter-Server bekommt seine Blätter von ihr.
        let ca = Arc::new(CaStore::load_or_create(&paths.ca_dir()).expect("the CA"));
        ca.write_bundle(None).expect("the CA bundle");
        let leaves = Arc::new(LeafCache::new(Arc::clone(&ca), 16));
        let stand_in = StandIn {
            leaves: Arc::clone(&leaves),
            seen: Arc::new(Mutex::new(Vec::new())),
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
            _dir: dir,
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
    let request = v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
            profile: "default".to_owned(),
            work_dir: work.display().to_string(),
            work_mode: "rw".to_owned(),
            command: vec!["claude".to_owned()],
            sandbox_mode: "direct".to_owned(),
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
        config_env: "CLAUDE_CONFIG_DIR",
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
