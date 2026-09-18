//! Der ganze Weg durch den echten Daemon (HUM-018).
//!
//! Dieser Test startet das gebaute Binary in einem eigenen XDG-Baum, schickt
//! eine HTTP-Anfrage in den Proxy-Socket, sieht das `Held`-Ereignis im
//! gRPC-Strom, entscheidet über gRPC und prüft, dass der wartende Client
//! daraufhin die Block-Antwort bekommt. Damit ist die Verdrahtung aus
//! `main.rs` belegt und nicht nur jede Hälfte für sich.
//!
//! Blockiert wird, nicht erlaubt: eine erlaubte Anfrage bräuchte ein
//! erreichbares Ziel, und ein Test, der das Netz braucht, ist kein Test,
//! sondern eine Wettervorhersage. Der Weg bis zur Entscheidung ist derselbe.
//!
//! Der Socket-Pfad muss in `sun_path` passen (108 Bytes), deshalb liegt das
//! Wegwerf-Verzeichnis unter `/tmp` und nicht unter einem womöglich tiefen
//! `TMPDIR`.
//!
//! Am Ende der Datei steht der Abschied (HUM-142): Ein Agent, der `SIGTERM`
//! abfängt, ist nach `Sandbox(Stop)` und nach dem Ende des Daemons in
//! beschränkter Zeit weg. Gemessen wird an seinen Prozessen in `/proc` und am
//! Protokoll des Daemons, nicht an dem, was der Ereignisstrom behauptet.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::process::ExitStatusExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::time::Duration;

use humanitl_ipc::{auth, client, v1};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::UnixStream;
use tokio_stream::StreamExt as _;

/// Der laufende Daemon samt seinem Wegwerf-Baum.
struct Daemon {
    dir: tempfile::TempDir,
    child: Child,
}

impl Daemon {
    /// Startet das gebaute Binary und wartet, bis beide Sockets stehen.
    fn start(hold_timeout_secs: u64) -> Self {
        Self::start_with(hold_timeout_secs, None)
    }

    /// Wie [`Daemon::start`], aber mit einem gesetzten `llm.endpoint`.
    fn start_with(hold_timeout_secs: u64, llm_endpoint: Option<&str>) -> Self {
        Self::build(hold_timeout_secs, llm_endpoint, false)
    }

    /// Wie [`Daemon::start`], mit der Fehlerausgabe in `daemon.log` statt auf
    /// dem Terminal.
    ///
    /// Das Protokoll ist hier eine Messung und keine Bequemlichkeit: Der
    /// Abschied sagt darin, ob er die Sandbox beendet hat und ob er Aufgaben
    /// abbrechen musste (`DAEMON_009`, HUM-142).
    fn start_logged(hold_timeout_secs: u64) -> Self {
        Self::build(hold_timeout_secs, None, true)
    }

    /// Legt den Wegwerf-Baum an und startet das Binary darin.
    fn build(hold_timeout_secs: u64, llm_endpoint: Option<&str>, log: bool) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("hum")
            .tempdir_in("/tmp")
            .expect("a short temporary directory for sun_path");
        for name in ["run", "data", "config", "home"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        let child = spawn_logging(dir.path(), hold_timeout_secs, llm_endpoint, log);
        Self { dir, child }
    }

    /// Was der Daemon dieses Laufs auf seine Fehlerausgabe geschrieben hat.
    ///
    /// Leer, wenn er nicht mit [`Daemon::start_logged`] gestartet wurde.
    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("daemon.log")).unwrap_or_default()
    }

    /// Beendet den Daemon und startet einen neuen im selben Baum.
    ///
    /// Der zweite Prozess hat eine leere Registry und eine neue Sitzung; was er
    /// über frühere Flows sagt, kann deshalb nur aus der Aufzeichnung kommen.
    async fn restart(&mut self, hold_timeout_secs: u64) {
        self.terminate();
        self.child = spawn(self.dir.path(), hold_timeout_secs, None);
        self.ready().await;
    }

    fn runtime(&self) -> PathBuf {
        self.dir.path().join("run").join("humanitl")
    }

    fn socket(&self) -> PathBuf {
        self.runtime().join("daemon.sock")
    }

    fn token_path(&self) -> PathBuf {
        self.runtime().join("token")
    }

    fn proxy_socket(&self) -> PathBuf {
        self.runtime().join("proxy").join("proxy.sock")
    }

    /// Wartet, bis Token, gRPC-Socket und Proxy-Socket da sind.
    async fn ready(&self) {
        for path in [self.token_path(), self.socket(), self.proxy_socket()] {
            await_path(&path).await;
        }
    }

    /// Beendet den Daemon mit `SIGTERM` und wartet auf sein Ende.
    fn terminate(&mut self) {
        self.signal_terminate();
        let status = self.child.wait().expect("the daemon must be reapable");
        assert!(status.success(), "SIGTERM is an orderly end: {status}");
    }

    /// Schickt `SIGTERM`, ohne auf das Ende zu warten.
    ///
    /// SIGTERM statt `Child::kill` (`SIGKILL`): nur der geordnete Weg räumt
    /// Socket und Token weg — und seit HUM-142 auch die Sandbox.
    fn signal_terminate(&self) {
        let pid = i32::try_from(self.child.id()).unwrap();
        // SAFETY: `kill` mit einer eigenen, noch nicht abgeernteten Kind-PID.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }

    /// Wartet höchstens `deadline` auf das Ende; `None`, wenn er dann noch
    /// läuft.
    ///
    /// Das Warten hat eine Frist, weil genau das die Messung ist: Ein Daemon,
    /// der nach `SIGTERM` nicht endet, soll diesen Test rot machen und nicht
    /// den Testläufer anhalten (HUM-142).
    fn wait_within(&mut self, deadline: Duration) -> Option<ExitStatus> {
        let started = std::time::Instant::now();
        loop {
            match self.child.try_wait() {
                Ok(Some(status)) => return Some(status),
                Ok(None) => {}
                Err(_) => return None,
            }
            if started.elapsed() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// `SIGTERM` und das befristete Warten in einem Schritt.
    fn terminate_within(&mut self, deadline: Duration) -> Option<ExitStatus> {
        self.signal_terminate();
        self.wait_within(deadline)
    }

    /// Ein Projektverzeichnis im Heimatverzeichnis dieses Laufs.
    ///
    /// Unter `HOME`, weil der Dienst nur von dort ein Projekt annimmt
    /// (`Inner::check_work_dir`).
    fn work_dir(&self) -> PathBuf {
        let work = self.dir.path().join("home").join("project");
        std::fs::create_dir_all(work.join(".git")).unwrap();
        work
    }

    /// Legt das mitgelieferte Profil dorthin, wo der Dienst zuerst sucht.
    fn install_profile(&self) {
        let profiles = self
            .dir
            .path()
            .join("config")
            .join("humanitl")
            .join("profiles")
            .join("sandbox");
        std::fs::create_dir_all(&profiles).unwrap();
        let bundled = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../profiles/sandbox")
            .join(format!("{PROFILE}.toml"));
        std::fs::copy(&bundled, profiles.join(format!("{PROFILE}.toml")))
            .unwrap_or_else(|err| panic!("{} is readable: {err}", bundled.display()));
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Startet das gebaute Binary in diesem XDG-Baum.
fn spawn(dir: &Path, hold_timeout_secs: u64, llm_endpoint: Option<&str>) -> Child {
    spawn_logging(dir, hold_timeout_secs, llm_endpoint, false)
}

/// Wie [`spawn`], und legt die Fehlerausgabe in `<dir>/daemon.log`, wenn `log`
/// wahr ist; sonst erbt sie wie bisher das Terminal.
fn spawn_logging(
    dir: &Path,
    hold_timeout_secs: u64,
    llm_endpoint: Option<&str>,
    log: bool,
) -> Child {
    let mut command = Command::new(env!("CARGO_BIN_EXE_humanitld"));
    command
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("HOME", dir.join("home"))
        .env("HUMANITL_HOLD__TIMEOUT_SECS", hold_timeout_secs.to_string());
    if let Some(endpoint) = llm_endpoint {
        command.env("HUMANITL_LLM__ENDPOINT", endpoint);
    }
    if log {
        let file = std::fs::File::create(dir.join("daemon.log")).expect("a log file");
        command.stderr(std::process::Stdio::from(file));
    }
    command.spawn().expect("the daemon binary must start")
}

/// Wartet höchstens zehn Sekunden auf eine Datei.
async fn await_path(path: &Path) {
    for _ in 0..1000 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("{} never appeared", path.display());
}

#[tokio::test]
async fn a_request_is_held_until_a_decision_arrives_over_grpc() {
    let mut daemon = Daemon::start(120);
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let info = grpc.get_info(()).await.unwrap().into_inner();
    assert_eq!(info.proto_major, humanitl_ipc::PROTO_MAJOR);
    assert!(
        !info.session_id.is_empty(),
        "the daemon runs one proxy session"
    );

    let mut events = grpc
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    // Der „Agent": eine gewöhnliche HTTP/1.1-Anfrage in den Proxy-Socket.
    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(b"GET /secret HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    // Warten, bis der Flow hängt.
    let flow_id = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = events.next().await.unwrap().unwrap();
            if let Some(v1::flow_event::Event::Held(held)) = event.event {
                break held.flow_id;
            }
        }
    })
    .await
    .expect("the request must be held within ten seconds");

    let response = grpc
        .decide(v1::DecideRequest {
            flow_ids: vec![flow_id.clone()],
            decision: Some(v1::decide_request::Decision::Block(
                v1::decide_request::Block {
                    note: "nicht ohne mich".to_owned(),
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert!(response.results[0].applied);

    // Der wartende Client bekommt die Block-Antwort, nicht das Ziel.
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), agent.read_to_end(&mut raw))
        .await
        .expect("the blocked client must be answered")
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 403"), "{text}");
    assert!(text.contains("X-Humanitl-Note: nicht ohne mich"), "{text}");
    assert!(text.contains(&flow_id), "{text}");

    // Und die Historie kennt ihn.
    let page = grpc
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .unwrap()
        .into_inner();
    let row = page
        .flows
        .iter()
        .find(|row| row.flow_id == flow_id)
        .expect("the decided flow is in the history");
    assert_eq!(row.decision, v1::DecisionKind::Block as i32);
    assert_eq!(row.authority.as_ref().unwrap().host, "example.com");

    drop(events);
    drop(grpc);
    daemon.terminate();

    assert!(!daemon.socket().exists(), "SIGTERM removes the socket");
    assert!(!daemon.token_path().exists(), "SIGTERM removes the token");
    assert!(
        !daemon.proxy_socket().exists(),
        "SIGTERM ends the proxy session"
    );
}

#[tokio::test]
async fn a_request_nobody_decides_runs_into_the_timeout_and_is_blocked() {
    let mut daemon = Daemon::start(1);
    daemon.ready().await;

    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(b"GET / HTTP/1.1\r\nHost: example.com\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();

    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), agent.read_to_end(&mut raw))
        .await
        .expect("the timeout must end the wait")
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 504"), "{text}");
    assert!(text.contains("reason: timeout"), "{text}");

    daemon.terminate();
}

#[tokio::test]
async fn a_second_daemon_refuses_and_leaves_the_first_one_alone() {
    let mut daemon = Daemon::start(120);
    daemon.ready().await;

    let output = Command::new(env!("CARGO_BIN_EXE_humanitld"))
        .env("XDG_RUNTIME_DIR", daemon.dir.path().join("run"))
        .env("XDG_DATA_HOME", daemon.dir.path().join("data"))
        .env("XDG_CONFIG_HOME", daemon.dir.path().join("config"))
        .env("HOME", daemon.dir.path().join("home"))
        .output()
        .expect("the second daemon must run and fail");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("DAEMON_003"), "{stderr}");

    // Der erste Daemon behält beides: seinen gRPC-Socket und den Socket, den
    // der Launcher in die Sandbox einhängen würde.
    assert!(daemon.socket().exists());
    assert!(daemon.proxy_socket().exists());
    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();
    assert!(
        grpc.get_info(()).await.is_ok(),
        "the first daemon serves on"
    );

    drop(grpc);
    daemon.terminate();
}

/// Der Body der Anfrage, die aufgezeichnet und danach wieder gelesen wird.
const RECORDED_BODY: &str = "{\"secret\":false}";

/// Schickt eine Anfrage mit Body, die niemand entscheidet, und wartet auf die
/// Antwort nach der Frist.
async fn post_and_time_out(daemon: &Daemon) {
    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(
            format!(
                "POST /notes HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{RECORDED_BODY}",
                RECORDED_BODY.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), agent.read_to_end(&mut raw))
        .await
        .expect("the timeout must end the wait")
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 504"), "{text}");
}

/// Die Zeile des aufgezeichneten Flows aus `ListFlows`.
async fn recorded_row(grpc: &mut client::Client) -> v1::FlowSummary {
    let page = grpc
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .unwrap()
        .into_inner();
    page.flows
        .iter()
        .find(|row| row.path == "/notes")
        .expect("the recorded flow is in the history")
        .clone()
}

/// Was der Daemon aufgezeichnet hat, überlebt ihn (HUM-026, HUM-027, HUM-031).
///
/// Der Beweis, dass `ListFlows`, `GetFlow` und `GetBody` aus der Aufzeichnung
/// lesen und nicht aus dem Speicher: Der zweite Prozess hat eine leere
/// Registry und eine neue Sitzung. Was er über den Flow von vorhin sagt, kann
/// er nur aus der Datenbank haben.
#[tokio::test]
async fn the_recording_outlives_the_daemon() {
    let mut daemon = Daemon::start(1);
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    // `Rules` ist kein `IPC_005` mehr: Der Daemon hat einen Regelspeicher
    // (HUM-027). Wie viele Regeln er nennt, ist hier gleichgültig — der
    // mitgelieferte Satz wird erst in HUM-038 gefüllt.
    grpc.rules(v1::RulesRequest {
        op: Some(v1::rules_request::Op::List(())),
    })
    .await
    .expect("the daemon answers Rules from its rule store");

    post_and_time_out(&daemon).await;
    let row = recorded_row(&mut grpc).await;
    assert_eq!(row.decision, v1::DecisionKind::TimedOut as i32);
    assert_eq!(row.request_size, RECORDED_BODY.len() as u64);

    let detail = grpc
        .get_flow(v1::FlowRef {
            flow_id: row.flow_id.clone(),
        })
        .await
        .expect("GetFlow answers from the recording")
        .into_inner();
    assert_eq!(detail.body_preview, RECORDED_BODY, "{detail:?}");
    assert!(!detail.findings_truncated, "the whole request was scanned");
    let domain = detail.domain.as_ref().expect("the catalog answers");
    assert_eq!(domain.apex, "example.com", "{domain:?}");
    assert_eq!(domain.seen_count, 1, "one request, one observation");
    let request = detail.request.as_ref().expect("the recorded request");
    assert!(
        request
            .headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("content-type")),
        "{:?}",
        request.headers
    );
    let body_ref = request.body.clone().expect("the request has a body");
    assert_eq!(body_ref.size, RECORDED_BODY.len() as u64);

    let mut chunks = grpc
        .get_body(body_ref)
        .await
        .expect("GetBody answers from the recording")
        .into_inner();
    let mut bytes = Vec::new();
    while let Some(chunk) = chunks.next().await {
        bytes.extend_from_slice(&chunk.unwrap().data);
    }
    assert_eq!(String::from_utf8_lossy(&bytes), RECORDED_BODY);

    // Und jetzt der eigentliche Punkt: neuer Prozess, leere Registry.
    drop(grpc);
    daemon.restart(1).await;
    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let after = recorded_row(&mut grpc).await;
    assert_eq!(after.flow_id, row.flow_id, "the same flow, a new process");
    assert_eq!(after.decision, v1::DecisionKind::TimedOut as i32);
    assert_eq!(after.request_size, RECORDED_BODY.len() as u64);
    assert_eq!(
        after.authority.as_ref().unwrap().host,
        "example.com",
        "{after:?}"
    );
    let detail = grpc
        .get_flow(v1::FlowRef {
            flow_id: row.flow_id.clone(),
        })
        .await
        .expect("GetFlow answers after the restart")
        .into_inner();
    assert_eq!(detail.body_preview, RECORDED_BODY, "{detail:?}");

    drop(grpc);
    daemon.terminate();
}

/// Schickt eine Anfrage an `host`, die niemand entscheidet, und wartet auf die
/// Antwort nach der Frist.
///
/// Nichts verlässt den Rechner: Der Flow läuft in die Frist und wird
/// geblockt, also wird `host` nie aufgelöst und nie verbunden.
async fn get_and_time_out(daemon: &Daemon, host: &str, path: &str) {
    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .await
        .unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), agent.read_to_end(&mut raw))
        .await
        .expect("the timeout must end the wait")
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 504"), "{text}");
}

/// Die registrierbare Domain erreicht beide Wege und überlebt den Neustart.
///
/// Der Apex ist eine Zusage über eine Zeile, und eine Zeile kommt auf zwei
/// Wegen an: als `Received` im Ereignisstrom und als Zeile aus `ListFlows`.
/// Sagen die beiden Verschiedenes, sieht ein Mensch je nach Fenster etwas
/// anderes; genau das war der Bruch, den HUM-091 behebt. Nach dem Neustart
/// kann der Wert nur aus der Spalte `apex` kommen, und `--filter apex:` muss
/// denselben String vergleichen, der in der Zeile steht.
#[tokio::test]
async fn the_apex_reaches_both_ways_and_survives_a_restart() {
    let mut daemon = Daemon::start(1);
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();
    let mut events = grpc
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    // `github.io` steht im privaten Abschnitt der Public Suffix List, also
    // gehört `a.b.github.io` zu `b.github.io`; eine IP hat keinen Apex.
    let targets = [
        ("a.b.github.io", "/one", "b.github.io"),
        ("api.github.com", "/two", "github.com"),
        ("192.168.1.50", "/three", ""),
    ];
    for (host, path, _) in targets {
        get_and_time_out(&daemon, host, path).await;
    }

    // Weg eins: der Ereignisstrom.
    let live: std::collections::HashMap<String, String> =
        tokio::time::timeout(Duration::from_secs(20), async {
            let mut seen = std::collections::HashMap::new();
            while seen.len() < targets.len() {
                let event = events.next().await.unwrap().unwrap();
                if let Some(v1::flow_event::Event::Received(received)) = event.event {
                    let summary = received.summary.expect("Received carries its row");
                    seen.insert(summary.flow_id.clone(), summary.apex);
                }
            }
            seen
        })
        .await
        .expect("three requests arrive within twenty seconds");

    // Weg zwei: die Liste. Für dieselbe `flow_id` derselbe String.
    let page = grpc
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .unwrap()
        .into_inner();
    for (host, path, apex) in targets {
        let row = page
            .flows
            .iter()
            .find(|row| row.path == path)
            .unwrap_or_else(|| panic!("{host} is in the history"));
        assert_eq!(row.apex, apex, "ListFlows: {host}");
        assert_eq!(
            live.get(&row.flow_id).map(String::as_str),
            Some(apex),
            "Subscribe and ListFlows disagree about {host}"
        );
    }

    // Neuer Prozess, leere Registry: Der Apex kann nur aus der Spalte kommen.
    drop(events);
    drop(grpc);
    daemon.restart(1).await;
    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let page = grpc
        .list_flows(v1::ListFlowsRequest::default())
        .await
        .unwrap()
        .into_inner();
    for (host, path, apex) in targets {
        let row = page
            .flows
            .iter()
            .find(|row| row.path == path)
            .unwrap_or_else(|| panic!("{host} survives the restart"));
        assert_eq!(row.apex, apex, "after the restart: {host}");
    }

    // Und der Filter vergleicht genau diesen String, nicht ein Suffix.
    let filtered = grpc
        .list_flows(v1::ListFlowsRequest {
            filter: "apex:b.github.io".to_owned(),
            ..v1::ListFlowsRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    let paths: Vec<&str> = filtered.flows.iter().map(|row| row.path.as_str()).collect();
    assert_eq!(paths, vec!["/one"], "apex: selects exactly the one row");

    let suffix = grpc
        .list_flows(v1::ListFlowsRequest {
            filter: "apex:github.io".to_owned(),
            ..v1::ListFlowsRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert!(
        suffix.flows.is_empty(),
        "apex: is exact; `github.io` is a public suffix, not a registrable domain"
    );

    drop(grpc);
    daemon.terminate();
}

/// Ein Flow, den niemand kennt, ist `NOT_FOUND` und kein leeres Detail.
#[tokio::test]
async fn a_flow_that_never_existed_is_not_found() {
    let mut daemon = Daemon::start(120);
    daemon.ready().await;
    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let status = grpc
        .get_flow(v1::FlowRef {
            flow_id: "0199c0ff-ee00-7000-8000-000000000001".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::NotFound, "{status}");

    let status = grpc
        .get_flow(v1::FlowRef {
            flow_id: "not-a-flow-id".to_owned(),
        })
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::InvalidArgument, "{status}");
    assert!(status.message().contains("IPC_004"), "{status}");

    drop(grpc);
    daemon.terminate();
}

/// Der konfigurierte LLM-Endpunkt wird zu einer Regel im laufenden Daemon
/// (HUM-039, HUM-037 Schritt 6).
///
/// Ohne diese Verdrahtung hält der Proxy jede Inferenz an, und der
/// Durchreich-Zweig samt `LLM_005` bleibt toter Code — grün getestet in den
/// Crate-Tests, wirkungslos im Programm. Der Test fragt deshalb den Daemon
/// selbst, nicht den Adapter: Er liest den Regelsatz über die `Rules`-RPC und
/// erwartet die Regel an erster Stelle, weil eine breite Blockregel sonst vor
/// ihr stünde.
#[tokio::test]
async fn the_configured_llm_endpoint_becomes_a_passthrough_rule() {
    let mut daemon = Daemon::start_with(120, Some("http://192.168.1.50:11434"));
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let response = grpc
        .rules(v1::RulesRequest {
            op: Some(v1::rules_request::Op::List(())),
        })
        .await
        .unwrap()
        .into_inner();

    let rule = response
        .rules
        .first()
        .expect("the rule set is not empty")
        .clone();
    assert!(
        rule.passthrough_llm,
        "the passthrough comes first; the bundled block rules must not shadow it: {:?}",
        response
            .rules
            .iter()
            .map(|r| (r.passthrough_llm, r.note.clone()))
            .collect::<Vec<_>>()
    );
    assert_eq!(rule.rule_id, "01920000-0000-7000-8000-0000000000ff");
    assert!(rule.bundled);
    assert!(rule.allow_private, "the model lives on a private address");
    assert_eq!(rule.action, v1::RuleAction::Allow as i32);

    let matcher = rule.matcher.expect("a matcher");
    // `HostPattern::Exact(HostName::Ip(..))`, nicht `HostPattern::Ip`: die
    // Spezifikation nennt genau diese Form, und sie schreibt sich ohne
    // Präfix (`backlog/sprint-3.md` HUM-039).
    assert_eq!(matcher.host, "192.168.1.50");
    assert_eq!(matcher.port, 11434);
    assert!(
        matcher.path_prefixes.iter().any(|p| p == "/api/chat"),
        "{:?}",
        matcher.path_prefixes
    );
    assert!(
        !matcher
            .path_prefixes
            .iter()
            .any(|p| "/api/pull".starts_with(p)),
        "pulling a model is not inference: {:?}",
        matcher.path_prefixes
    );

    daemon.terminate();
}

/// Ohne `llm.endpoint` gibt es keine Durchreiche — es wird gefragt.
#[tokio::test]
async fn without_an_endpoint_there_is_no_passthrough_rule() {
    let mut daemon = Daemon::start(120);
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let response = grpc
        .rules(v1::RulesRequest {
            op: Some(v1::rules_request::Op::List(())),
        })
        .await
        .unwrap()
        .into_inner();

    assert!(
        !response.rules.iter().any(|rule| rule.passthrough_llm),
        "no endpoint, no exception"
    );

    daemon.terminate();
}

// ---------------------------------------------------------------------------
// `--allow-test-ca` (HUM-087)
// ---------------------------------------------------------------------------

/// Ein XDG-Baum mit dieser `config.toml`.
///
/// Über die Datei und nicht über eine Umgebungsvariable: Genau diesen Weg
/// nimmt eine Konfiguration im Alltag.
fn tree_with_config(toml: &str) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("hum")
        .tempdir_in("/tmp")
        .expect("a short temporary directory for sun_path");
    for name in ["run", "data", "config", "home"] {
        std::fs::create_dir(dir.path().join(name)).unwrap();
    }
    let config = dir.path().join("config").join("humanitl");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(config.join("config.toml"), toml).unwrap();
    dir
}

/// Ein XDG-Baum mit einer `config.toml`, die `resolver.test_ca` setzt.
///
/// Derselbe Weg wie oben, und genau er darf das Vertrauen nicht allein
/// herstellen.
fn tree_with_test_ca(test_ca: &Path) -> tempfile::TempDir {
    tree_with_config(&format!(
        "[resolver]\ntest_ca = \"{}\"\n",
        test_ca.display()
    ))
}

/// Startet das Binary in `dir` mit den zusätzlichen Argumenten und gibt seine
/// Fehlerausgabe zurück, sobald es geendet hat.
///
/// `until_ready` beendet einen Daemon, der hochkommt, mit `SIGTERM`; ohne das
/// wird auf das Ende gewartet, das der Daemon von sich aus findet.
///
/// Gewartet wird mit Frist. Ohne sie bekäme ein Daemon, der wider Erwarten
/// stehen bleibt, keinen roten Test, sondern einen Lauf, der nie endet — und
/// ein Test, der hängt, statt zu scheitern, sagt nichts.
fn run_daemon(dir: &Path, args: &[&str], until_ready: bool) -> (ExitStatus, String) {
    run_daemon_in(dir, args, until_ready, None)
}

/// Wie [`run_daemon`], aber mit einem Arbeitsverzeichnis für den Daemon.
///
/// Nur ein Test braucht das, und er braucht es zwingend: Ein relativer Pfad in
/// `resolver.test_ca` wird gegen genau dieses Verzeichnis aufgelöst, und ohne
/// die Möglichkeit, es zu setzen, ließe sich der Fall nicht messen.
fn run_daemon_in(
    dir: &Path,
    args: &[&str],
    until_ready: bool,
    cwd: Option<&Path>,
) -> (ExitStatus, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_humanitld"));
    command
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("HOME", dir.join("home"))
        .args(args)
        .stderr(std::process::Stdio::piped());
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let mut child = command.spawn().expect("the daemon binary must start");
    if until_ready {
        let socket = dir.join("run").join("humanitl").join("daemon.sock");
        for _ in 0..1000 {
            if socket.exists() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let pid = i32::try_from(child.id()).unwrap();
        // SAFETY: `kill` mit einer eigenen, noch nicht abgeernteten Kind-PID.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
    }
    for _ in 0..1000 {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if child.try_wait().unwrap().is_none() {
        let _ = child.kill();
        let _ = child.wait();
        panic!(
            "the daemon was still running ten seconds after `humanitld {}`",
            args.join(" ")
        );
    }
    let out = child
        .wait_with_output()
        .expect("the daemon must be reapable");
    (
        out.status,
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// Wahr, wenn der Daemon bis zu unserem Signal gelaufen ist.
///
/// Zwei Ausgänge zählen dazu, und beide heißen dasselbe: Er endete geordnet
/// (Status 0), oder das Signal traf ihn, bevor sein Handler stand, und der
/// Kernel hat ihn beendet (Signal 15). Der Socket erscheint beim Binden, der
/// Handler wird erst danach beim ersten Pollen der Abschaltung eingehängt; wer
/// unmittelbar nach dem Socket signalisiert, trifft manchmal in diese Lücke.
///
/// Was **nicht** dazuzählt, ist genau der Fall, um den es hier geht: ein
/// Daemon, der von selbst mit einem Fehler endet. Der käme mit Status 1
/// zurück, und den lässt diese Funktion durchfallen.
fn ran_until_the_signal(status: ExitStatus) -> bool {
    status.success() || status.signal() == Some(libc::SIGTERM)
}

/// Eine echte CA in einem Wegwerf-Verzeichnis; ihr `ca.crt` ist die Testwurzel.
fn a_root() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let store = humanitl_proxy::ca::CaStore::load_or_create(&tmp.path().join("ca")).unwrap();
    let path = store.cert_path();
    (tmp, path)
}

/// Derselbe Baum, einmal mit und einmal ohne das Flag.
///
/// Gemessen wird am Protokoll des Daemons und nicht an einem Ausbleiben: Mit
/// dem Flag steht dort die Zeile mit `roots` und dem Pfad, ohne das Flag der
/// Befund `CONFIG_011`. Beides kommt vom Daemon selbst.
#[test]
fn a_test_ca_is_only_trusted_with_the_flag() {
    let (_ca, root) = a_root();

    let dir = tree_with_test_ca(&root);
    let (status, log) = run_daemon(dir.path(), &["--allow-test-ca"], true);
    assert!(
        ran_until_the_signal(status),
        "the daemon has to run until the signal, not end on its own: {status}\n{log}"
    );
    let line = log
        .lines()
        .find(|line| line.contains("\"roots\":1"))
        .unwrap_or_else(|| panic!("the start has to name how many roots it trusts: {log}"));
    assert!(
        line.contains(&root.display().to_string()),
        "the start has to name the file: {line}"
    );
    assert!(
        line.contains("\"level\":\"WARN\""),
        "an extra trust anchor is a warning, not a note in passing: {line}"
    );
    assert!(
        !log.contains("CONFIG_011"),
        "flag and key are both there, so there is nothing to warn about: {log}"
    );

    let dir = tree_with_test_ca(&root);
    let (status, log) = run_daemon(dir.path(), &[], true);
    assert!(
        ran_until_the_signal(status),
        "the daemon has to run until the signal, not end on its own: {status}\n{log}"
    );
    assert!(
        log.contains("CONFIG_011"),
        "a key without the flag has to be said out loud: {log}"
    );
    assert!(
        !log.contains("\"roots\":"),
        "without the flag no root is trusted, so no line claims one: {log}"
    );
}

/// Ein relativer Pfad in `resolver.test_ca` beendet den Start, auch wenn genau
/// dort eine tadellose Wurzel liegt.
///
/// Der Aufbau ist der Angriff: Ein präpariertes Projektverzeichnis mit einer
/// eigenen `ca.crt` darin, der Daemon in diesem Verzeichnis gestartet, und in
/// der Konfiguration steht nur der Name. Würde der Pfad gegen das
/// Arbeitsverzeichnis aufgelöst, entschiede das Verzeichnis, welcher Wurzel
/// der Daemon vertraut — das Flag bliebe nötig, die Datei käme aus dem Projekt.
#[test]
fn a_relative_test_ca_stops_the_start_even_next_to_a_valid_root() {
    let project = tempfile::Builder::new()
        .prefix("hum-project")
        .tempdir_in("/tmp")
        .expect("a project directory");
    let store = humanitl_proxy::ca::CaStore::load_or_create(&project.path().join("ca"))
        .expect("a certificate authority in the project directory");
    std::fs::copy(store.cert_path(), project.path().join("ca.crt"))
        .expect("a perfectly good root, lying in the project directory");

    let dir = tree_with_test_ca(Path::new("ca.crt"));
    let (status, log) = run_daemon_in(
        dir.path(),
        &["--allow-test-ca"],
        false,
        Some(project.path()),
    );

    assert_eq!(status.code(), Some(1), "{log}");
    assert!(
        log.contains("CONFIG_012"),
        "a relative path is its own refusal, not a missing file: {log}"
    );
    assert!(
        !log.contains("CONFIG_010"),
        "the file is not the problem, the path is: {log}"
    );
    assert!(
        !log.contains("\"roots\":"),
        "nothing may be trusted on this start: {log}"
    );

    let runtime = dir.path().join("run").join("humanitl");
    assert!(
        !runtime.join("daemon.sock").exists(),
        "a daemon that refuses to start leaves no gRPC socket"
    );
    assert!(
        !runtime.join("proxy").join("proxy.sock").exists(),
        "and no proxy socket either"
    );
}

/// Eine nicht leere Tabelle `resolver.overrides` steht beim Start im
/// Protokoll, mit Code und Namen, und der Daemon läuft trotzdem.
///
/// Der Hebel beantwortet Namen aus der Konfiguration, statt zu fragen; ohne
/// diese Zeile stünde er unbemerkt in einem Alltagslauf
/// (`backlog/CONVENTIONS.md` 4.22, HUM-024).
#[test]
fn a_table_of_fixed_names_is_announced_at_the_start() {
    let dir = tree_with_config("[resolver.overrides]\n\"registry.npmjs.test\" = \"127.0.0.1\"\n");

    let (_status, log) = run_daemon(dir.path(), &[], true);

    assert!(log.contains("CONFIG_016"), "{log}");
    assert!(log.contains("registry.npmjs.test"), "{log}");
    assert!(log.contains("\"level\":\"WARN\""), "not INFO: {log}");
    assert!(
        dir.path()
            .join("run")
            .join("humanitl")
            .join("daemon.sock")
            .exists()
            || log.contains("listening"),
        "the warning does not stop the start: {log}"
    );
}

/// Ohne die Tabelle sagt der Start nichts über sie.
///
/// Der Gegenfall zum Test darüber: Eine Zeile, die immer steht, sagt nichts.
#[test]
fn without_fixed_names_the_start_stays_quiet_about_them() {
    let dir = tree_with_config("");

    let (_status, log) = run_daemon(dir.path(), &[], true);

    assert!(!log.contains("CONFIG_016"), "{log}");
}

/// Eine unbrauchbare Testwurzel beendet den Start, und zwar bevor ein Socket
/// entsteht.
#[test]
fn a_broken_test_ca_stops_the_start() {
    let tmp = tempfile::tempdir().unwrap();
    let broken = tmp.path().join("broken.pem");
    std::fs::write(&broken, b"-----BEGIN CERTIFICATE-----\nnope\n").unwrap();

    let dir = tree_with_test_ca(&broken);
    let (status, log) = run_daemon(dir.path(), &["--allow-test-ca"], false);

    assert_eq!(status.code(), Some(1), "{log}");
    assert!(log.contains("CONFIG_010"), "{log}");
    assert!(
        log.contains(&broken.display().to_string()),
        "the why has to name the path: {log}"
    );
    assert!(
        log.contains("openssl x509 -in "),
        "the fix has to be a command a person can paste: {log}"
    );

    let runtime = dir.path().join("run").join("humanitl");
    assert!(
        !runtime.join("daemon.sock").exists(),
        "a daemon that refuses to start leaves no gRPC socket"
    );
    assert!(
        !runtime.join("proxy").join("proxy.sock").exists(),
        "and no proxy socket either"
    );
}

// ---------------------------------------------------------------------------
// Die Audit-Kette (HUM-050)
// ---------------------------------------------------------------------------

/// Das Audit-Log eines beendeten Daemons, mit dem Schlüssel und den Ankern,
/// die er selbst abgelegt hat.
struct AuditTrail {
    text: String,
    key: [u8; 32],
    anchors: Vec<humanitl_audit::Anchor>,
}

impl AuditTrail {
    fn of(dir: &Path) -> Self {
        let data = dir.join("data").join("humanitl");
        let text = std::fs::read_to_string(data.join("audit").join("audit.jsonl"))
            .expect("the daemon wrote an audit log");
        let key: [u8; 32] = std::fs::read(data.join("keys").join("audit.key"))
            .expect("the daemon created its audit key")
            .try_into()
            .expect("an audit key is 32 bytes");
        let anchors = humanitl_recorder::read_anchors(&data.join("humanitl.db"))
            .unwrap()
            .into_iter()
            .map(|anchor| humanitl_audit::Anchor {
                seq: anchor.seq,
                hash: anchor.hash,
                ts: anchor.ts,
            })
            .collect();
        Self { text, key, anchors }
    }

    fn records(&self) -> Vec<humanitl_audit::AuditRecord> {
        self.text
            .lines()
            .map(|line| humanitl_audit::AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect()
    }

    fn kinds(&self) -> Vec<String> {
        self.records()
            .into_iter()
            .map(|record| record.body.kind)
            .collect()
    }

    /// Prüft einen Text, als stünde er in der Datei, mit Schlüssel und Ankern
    /// des Daemons.
    fn verify(&self, text: &str) -> humanitl_audit::VerifyReport {
        humanitl_audit::AuditVerifier::verify_reader(
            std::io::Cursor::new(text.as_bytes()),
            Some(&self.key),
            &self.anchors,
        )
        .unwrap()
    }
}

/// Schickt eine Anfrage mit Body an `target`, die niemand entscheidet, und
/// wartet auf die Antwort nach der Frist.
async fn send_and_time_out(daemon: &Daemon, method: &str, target: &str, body: &str) {
    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(
            format!(
                "{method} {target} HTTP/1.1\r\nHost: example.com\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(20), agent.read_to_end(&mut raw))
        .await
        .expect("the timeout must end the wait")
        .unwrap();
    assert!(String::from_utf8_lossy(&raw).starts_with("HTTP/1.1 504"));
}

/// Eine Sitzung, eine Anfrage, ein geordnetes Ende: das Audit-Log danach.
async fn one_session(method: &str, target: &str, body: &str) -> AuditTrail {
    let mut daemon = Daemon::start(1);
    daemon.ready().await;
    send_and_time_out(&daemon, method, target, body).await;
    daemon.terminate();
    AuditTrail::of(daemon.dir.path())
}

/// Der Body trägt eine Mailadresse; im Log steht davon nichts, nicht einmal
/// der Feldname. Die Datei ist dabei eine vollständige, prüfbare Kette mit
/// allen Records einer Sitzung (Akzeptanzkriterien 1 und 2 von HUM-050).
#[tokio::test]
async fn decided_event_produces_record_without_payload() {
    const MAIL: &str = "alice.wonder@example.org";
    let body = format!("{{\"kontakt\":\"{MAIL}\"}}");
    let trail = one_session("POST", "/contact", &body).await;

    let kinds = trail.kinds();
    for kind in [
        "daemon.started",
        "session.started",
        "flow.received",
        "flow.decided",
        "session.ended",
        "daemon.stopped",
        "audit.anchor",
    ] {
        assert!(
            kinds.iter().any(|seen| seen == kind),
            "{kind} missing: {kinds:?}"
        );
    }
    assert_eq!(kinds.last().map(String::as_str), Some("audit.anchor"));

    // Die Suche nach dem Klartext liefert null Treffer (`grep -c` in der
    // Spezifikation), und auch der Body als Ganzes steht nirgends.
    assert_eq!(trail.text.matches(MAIL).count(), 0, "{}", trail.text);
    assert!(!trail.text.contains("kontakt"), "{}", trail.text);
    assert!(!trail.text.contains("/contact"), "{}", trail.text);

    let records = trail.records();
    let received = records
        .iter()
        .find(|record| record.body.kind == "flow.received")
        .unwrap();
    assert_eq!(received.body.data["method"], "POST");
    assert_eq!(received.body.data["host"], "example.com");
    assert_eq!(received.body.data["size"], body.len());
    assert!(
        received.body.data["findings_kinds"]
            .as_array()
            .unwrap()
            .iter()
            .any(|kind| kind == "email"),
        "the kind of the finding is named, its value is not: {:?}",
        received.body.data
    );
    let decided = records
        .iter()
        .find(|record| record.body.kind == "flow.decided")
        .unwrap();
    assert_eq!(decided.body.data["decision"], "timed_out");
    assert_eq!(decided.body.data["flow"], received.body.data["flow"]);

    // Und die Kette hält, mit dem Schlüssel und den Ankern des Daemons, ohne
    // unverankertes Ende.
    let report = trail.verify(&trail.text);
    assert!(report.is_ok(), "{report:?}");
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

/// Ein Token in der Query steht nicht im Log, nur die Prüfsumme des Pfads.
///
/// Die Spezifikation nimmt `token=abc`. Der Wert hier hat Buchstaben, die in
/// keinem Hex vorkommen: Drei Hex-Ziffern hintereinander stehen in einer
/// Datei voller Hashes mit hoher Wahrscheinlichkeit irgendwo, und dann würde
/// der Test ohne jedes Leck rot.
#[tokio::test]
async fn path_is_hashed() {
    const TARGET: &str = "/search?q=rust&token=quux-geheim-7";
    let trail = one_session("GET", TARGET, "").await;
    assert!(!trail.text.contains("quux-geheim-7"), "{}", trail.text);
    assert!(!trail.text.contains("/search"), "{}", trail.text);
    let received = trail
        .records()
        .into_iter()
        .find(|record| record.body.kind == "flow.received")
        .unwrap();
    assert_eq!(
        received.body.data["path_hash"],
        humanitl_audit::sha256_hex(TARGET.as_bytes())
    );
}

/// ESC-5 `audit_delete_is_detected`: Ein Eintrag aus der Mitte einer echten
/// Kette fehlt, und die Prüfung meldet den ersten Record danach als Bruch.
#[tokio::test]
async fn audit_delete_is_detected() {
    let trail = one_session("GET", "/", "").await;
    assert!(
        trail.verify(&trail.text).is_ok(),
        "the untouched chain holds"
    );

    let mut lines: Vec<&str> = trail.text.lines().collect();
    assert!(lines.len() >= 5, "{}", trail.text);
    let removed = humanitl_audit::AuditRecord::from_line(lines[2].as_bytes()).unwrap();
    lines.remove(2);
    let tampered = format!("{}\n", lines.join("\n"));
    assert_eq!(
        trail.verify(&tampered).status,
        humanitl_audit::VerifyStatus::Broken {
            first_bad_seq: removed.body.seq + 1,
            reason: humanitl_audit::BreakReason::SeqGap,
        }
    );
}

/// ESC-5 `audit_truncate_is_detected`: Das Ende einer geordnet beendeten Kette
/// ist verankert, in der Datei und in `SQLite`. Wer es abschneidet, schneidet
/// unter einen Anker, und das ist ein Bruch, keine Warnung.
#[tokio::test]
async fn audit_truncate_is_detected() {
    let trail = one_session("GET", "/", "").await;
    let lines: Vec<&str> = trail.text.lines().collect();
    let last = humanitl_audit::AuditRecord::from_line(lines[lines.len() - 1].as_bytes()).unwrap();
    assert!(
        trail
            .anchors
            .iter()
            .any(|anchor| anchor.seq == last.body.seq),
        "a daemon that stopped in order anchored its last record"
    );

    for cut in [1_usize, 2] {
        let kept = &lines[..lines.len() - cut];
        let tampered = format!("{}\n", kept.join("\n"));
        assert_eq!(
            trail.verify(&tampered).status,
            humanitl_audit::VerifyStatus::Broken {
                first_bad_seq: last.body.seq - u64::try_from(cut).unwrap(),
                reason: humanitl_audit::BreakReason::TruncatedBelowAnchor {
                    anchor_seq: last.body.seq
                },
            },
            "cutting {cut} line(s)"
        );
    }
}

// ---------------------------------------------------------------------------
// Die Notiz einer Entscheidung (HUM-117)
// ---------------------------------------------------------------------------

/// Der Satz, den der Mensch beim Blocken an den Agenten richtet.
const HUMAN_NOTE: &str = "use PyPI";

/// Die Zeile des geblockten Flows, sobald die Aufzeichnung sie führt.
///
/// Mit Aufzeichnung beantwortet `ListFlows` jede Seite aus `SQLite`
/// (`recorded_page` in `ipc/src/server.rs`), der Schreiber arbeitet aber
/// nebenläufig. Ohne dieses Warten hinge der Test daran, wer zuerst fertig
/// ist, und ein Test, der ein Rennen abwartet, misst das Rennen.
///
/// `when` steht in der Fehlermeldung, damit beide Aufrufstellen — vor und nach
/// dem Neustart — auseinanderzuhalten sind.
async fn blocked_row(grpc: &mut client::Client, flow_id: &str, when: &str) -> v1::FlowSummary {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let page = grpc
                .list_flows(v1::ListFlowsRequest::default())
                .await
                .unwrap()
                .into_inner();
            let row = page.flows.iter().find(|row| {
                row.flow_id == flow_id && row.decision == v1::DecisionKind::Block as i32
            });
            if let Some(row) = row {
                return row.clone();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_elapsed| {
        panic!("the blocked flow reaches ListFlows within ten seconds, {when}")
    })
}

/// Fragt den Meta-Endpunkt über den Proxy-Socket, so wie ein Agent es täte.
///
/// `humanitl.internal` wird nie aufgelöst und nie verbunden; der Proxy
/// beantwortet den Namen selbst (ADR-014). Zurück kommt die ganze Antwort
/// samt Statuszeile.
async fn meta_get(daemon: &Daemon, path: &str) -> String {
    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(
            format!("GET {path} HTTP/1.1\r\nHost: humanitl.internal\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await
        .unwrap();
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), agent.read_to_end(&mut raw))
        .await
        .expect("the meta endpoint answers within ten seconds")
        .unwrap();
    String::from_utf8_lossy(&raw).into_owned()
}

/// Was der neu gestartete Daemon über den geblockten Flow sagt.
///
/// Die zweite Hälfte des Tests darunter, als eigene Funktion: Der Prozess ist
/// ein anderer, die Verbindung ist eine neue, und was hier geprüft wird, kann
/// aus nichts anderem als der Aufzeichnung stammen.
async fn the_note_after_the_restart(daemon: &Daemon, flow_id: &str) {
    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();

    let after = blocked_row(&mut grpc, flow_id, "after the restart").await;
    assert_eq!(
        after.decision_note, HUMAN_NOTE,
        "the note came out of the column, not out of the registry: {after:?}"
    );
    let detail = grpc
        .get_flow(v1::FlowRef {
            flow_id: flow_id.to_owned(),
        })
        .await
        .expect("GetFlow answers after the restart")
        .into_inner();
    assert_eq!(
        detail.decision_note, HUMAN_NOTE,
        "GetFlow after the restart: {detail:?}"
    );
    assert_eq!(
        detail
            .summary
            .as_ref()
            .map(|row| row.decision_note.as_str()),
        Some(HUMAN_NOTE),
        "detail and row say the same thing: {detail:?}"
    );

    // Und die Sitzungsgrenze steht auch über den Neustart hinweg.
    let why = meta_get(daemon, &format!("/why/{flow_id}")).await;
    assert!(
        why.starts_with("HTTP/1.1 404"),
        "a new session does not get the flows of the old one: {why}"
    );
}

/// Was ein Mensch beim Blocken schreibt, überlebt den Daemon (HUM-117).
///
/// Der Weg geht durch den echten Prozess und nicht durch einen Fake: eine
/// Anfrage in den Proxy-Socket, das `Held`-Ereignis über gRPC, der Block mit
/// Notiz über `Decide`, die 403-Antwort an den wartenden Agenten — und danach
/// derselbe Baum, ein neuer Prozess. Der zweite Prozess hat eine leere
/// Registry und eine neue Sitzung; was er über die Notiz sagt, kann er nur aus
/// der Spalte `decision_note` haben, die `V8__decision_note.sql` angelegt hat.
///
/// `/why/<flow-id>` wird auf beiden Seiten des Neustarts gemessen, und die
/// beiden Antworten sind verschieden: vorher die Zeile aus der Registry,
/// nachher `404`. Der Rückfall auf die Aufzeichnung beantwortet nur Flows der
/// eigenen Sitzung (`backlog/CONVENTIONS.md` 4.24), und ein neu gestarteter
/// Daemon legt eine neue Sitzung an (`SessionId::new` in
/// `humanitld/src/main.rs`). Eine Sandbox ändert daran nichts: die Sitzung
/// gehört dem Prozess, nicht dem Agenten, und mit dem Prozess endet sie. Das
/// `404` ist hier also die Zusage und kein Mangel; was der Rückfall auf die
/// Aufzeichnung trägt, ist ein Flow, den die Registry innerhalb einer
/// laufenden Sitzung nicht mehr hält.
#[tokio::test]
async fn the_note_of_a_human_block_outlives_the_daemon() {
    let mut daemon = Daemon::start(120);
    daemon.ready().await;

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();
    let mut events = grpc
        .subscribe(v1::SubscribeRequest::default())
        .await
        .unwrap()
        .into_inner();

    let mut agent = UnixStream::connect(daemon.proxy_socket()).await.unwrap();
    agent
        .write_all(
            b"GET /simple/requests/ HTTP/1.1\r\nHost: files.example.com\r\n\
              Connection: close\r\n\r\n",
        )
        .await
        .unwrap();

    let flow_id = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let event = events.next().await.unwrap().unwrap();
            if let Some(v1::flow_event::Event::Held(held)) = event.event {
                break held.flow_id;
            }
        }
    })
    .await
    .expect("the request must be held within ten seconds");

    let response = grpc
        .decide(v1::DecideRequest {
            flow_ids: vec![flow_id.clone()],
            decision: Some(v1::decide_request::Decision::Block(
                v1::decide_request::Block {
                    note: HUMAN_NOTE.to_owned(),
                },
            )),
            ..v1::DecideRequest::default()
        })
        .await
        .unwrap()
        .into_inner();
    assert!(response.results[0].applied);

    // Der Agent liest die Notiz so, wie er sie immer gelesen hat.
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), agent.read_to_end(&mut raw))
        .await
        .expect("the blocked client must be answered")
        .unwrap();
    let text = String::from_utf8_lossy(&raw);
    assert!(text.starts_with("HTTP/1.1 403"), "{text}");
    let (head, _) = text
        .split_once("\r\n\r\n")
        .expect("the answer has a header block");
    assert!(
        head.lines()
            .any(|line| line == format!("X-Humanitl-Note: {HUMAN_NOTE}")),
        "the note is a header of its own, not only text in the body: {text}"
    );

    let before = blocked_row(&mut grpc, &flow_id, "before the restart").await;
    assert_eq!(before.decision_note, HUMAN_NOTE, "ListFlows: {before:?}");
    let why = meta_get(&daemon, &format!("/why/{flow_id}")).await;
    assert!(
        why.starts_with("HTTP/1.1 200"),
        "/why answers the running session: {why}"
    );
    assert!(
        why.ends_with(&format!("decision=block reason=user note={HUMAN_NOTE}\n")),
        "/why answers from the registry while the session runs: {why}"
    );

    // Neuer Prozess, leere Registry, neue Sitzung.
    drop(events);
    drop(grpc);
    daemon.restart(120).await;
    the_note_after_the_restart(&daemon, &flow_id).await;

    daemon.terminate();
}

// ---------------------------------------------------------------------------
// Der Abschied (HUM-142)
// ---------------------------------------------------------------------------

/// Das mitgelieferte Profil, mit dem diese Tests starten: dasselbe, das im
/// Produkt startet.
const PROFILE: &str = "default";

/// Wie lange nach `Sandbox(Stop)` höchstens vergehen darf, bis nichts mehr
/// läuft: die Gnadenfrist des Agenten plus eine Sekunde für Signal, Abbau des
/// Namensraums und das Einsammeln.
///
/// Die Frist kommt aus derselben Konstante wie im Produkt; eine eigene Zahl
/// hier wäre eine zweite Wahrheit über dieselbe Frist.
const STOP_DEADLINE: Duration = Duration::from_secs(humanitl_sandbox::KILL_GRACE.as_secs() + 1);

/// Die Zeile des Dienstes über das Ende der Sandbox (`humanitl_ipc::sandbox`).
const SANDBOX_ENDED: &str = "sandbox ended";

/// Die Zeile, mit der der gRPC-Dienst seinen Ausklang abschließt
/// (`humanitl_ipc::server`); sie kommt nach `SHUTDOWN_GRACE`.
const SERVICE_STOPPED: &str = "stopped, socket and token removed";

/// Wie lange der Daemon nach `SIGTERM` höchstens braucht, bis sein Prozess weg
/// ist: die Frist des Abschieds (11 s) plus die Frist für die Aufgaben (5 s)
/// plus vier Sekunden für alles, was dazwischen noch auf die Platte geht
/// (Aufzeichnung, Audit-Log).
const DAEMON_DEADLINE: Duration = Duration::from_secs(20);

/// Ein Agent, der `SIGTERM` abfängt — das Verhalten eines Vollbild-TUI, das am
/// 2026-09-07 zwei M3-Läufe zum Stehen gebracht hat.
///
/// **Was der Trap hier belegt und was nicht.** `SandboxHandle::terminate`
/// schickt sein `SIGTERM` an den Sandbox-Prozess auf dem Wirt und nicht an den
/// Agenten darin; dieser Prozess hat dafür keinen eigenen Handler, endet, und
/// mit ihm endet der PID-Namensraum. Der Trap des Agenten hält den Abschied
/// deshalb **nicht** auf, und die beiden Tests hier messen die Eskalation auf
/// `SIGKILL` nicht. Sie messen, dass der Abschied überhaupt stattfindet, wann
/// er beginnt, und dass danach nichts übrig ist. Die Eskalation misst
/// `a_child_that_ignores_sigterm_is_killed_after_the_grace` in
/// `daemon/crates/sandbox/src/handle.rs`, an einem Kind ohne Sandbox
/// dazwischen. Der Trap bleibt trotzdem stehen: Er ist der Agent aus der
/// Messung vom 2026-09-07, und ein Agent, der von selbst geht, wäre hier der
/// leichtere Fall.
///
/// Das Skript steht als **ein** Element in der Kommandozeile des
/// Sandbox-Prozesses und in der des Agenten darin ([`marked_processes`]); ein
/// PID-Namensraum verbirgt keine Prozesse vor dem Wirt, also findet der Test
/// beide.
fn deaf_agent(marker: &str) -> Vec<String> {
    vec!["/bin/sh".to_owned(), "-c".to_owned(), agent_script(marker)]
}

/// Das Skript des Agenten, Zeichen für Zeichen — der Schlüssel, an dem der
/// Test seine Prozesse in `/proc` wiedererkennt.
fn agent_script(marker: &str) -> String {
    format!("trap '' TERM; : {marker}; while :; do sleep 1; done")
}

/// Die Anfrage, die diese Sandbox startet.
fn start_request(work_dir: &Path, marker: &str) -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Start(v1::sandbox_request::Start {
            profile: PROFILE.to_owned(),
            work_dir: work_dir.display().to_string(),
            work_mode: "rw".to_owned(),
            command: deaf_agent(marker),
            session_profile: String::new(),
            ask_mode: String::new(),
            cli_overrides: Vec::new(),
        })),
    }
}

/// Die Anfrage, die sie beendet.
fn stop_request() -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Stop(())),
    }
}

/// Ob der Shim gebaut ist; ohne ihn gäbe es keinen Start, den man messen
/// könnte.
///
/// Ob das Sandbox-Backend auf dieser Maschine läuft, fragt dieser Test nicht
/// vorab: Der Start selbst sagt es (`SANDBOX_001` bis `SANDBOX_003`, siehe
/// [`Started`]), und zwar über denselben Weg, den auch ein Mensch nimmt.
///
/// **Unter `CI` ist das Fehlen ein Fehler und kein Grund zu überspringen** —
/// dieselbe Regel wie in `crates/ipc/tests/sandbox_start.rs`: Ein Test, der
/// zurückkehrt, gilt dem Testläufer als bestanden, und die Zusage von HUM-142
/// wäre nie geprüft worden.
fn shim_is_built() -> bool {
    if !shim_next_to_the_daemon() {
        return refuse_under_ci(
            "humanitl-shim is not built next to the daemon binary; build the workspace first \
             (cargo build --workspace --all-targets)",
        );
    }
    true
}

/// Ob der Shim dort liegt, wo der Dienst ihn sucht: neben dem Daemon.
fn shim_next_to_the_daemon() -> bool {
    Path::new(env!("CARGO_BIN_EXE_humanitld"))
        .parent()
        .is_some_and(|dir| dir.join("humanitl-shim").is_file())
}

/// Meldet, warum dieser Test nicht laufen kann — und scheitert unter `CI`.
fn refuse_under_ci(why: &str) -> bool {
    assert!(
        std::env::var_os("CI").is_none(),
        "under CI this test must run: {why}"
    );
    eprintln!("skipping: {why}");
    false
}

/// Ob dieser Prozess noch läuft.
///
/// Ein Zombie zählt als beendet: Er hat kein Programm mehr, nur noch einen
/// Eintrag, den sein Elternprozess abholt. Ohne diese Unterscheidung hinge der
/// Test an der Frage, wer einen verwaisten Sandbox-Prozess einsammelt.
fn alive(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // `pid (comm) state ...`; der Name kann Klammern enthalten, der Zustand
    // steht hinter der letzten schließenden.
    let rest = &stat[stat.rfind(')').map_or(0, |at| at + 1)..];
    rest.split_whitespace().next().unwrap_or("Z") != "Z"
}

/// Alle Prozesse, die das Skript dieses Laufs als eigenes Argument tragen.
///
/// **Verglichen wird ein ganzes Argument, kein Teilstück der Zeile.** Eine
/// Kommandozeile in `/proc` ist eine Folge von Argumenten, getrennt durch
/// `NUL`; gesucht wird eines, das Zeichen für Zeichen dem Skript aus
/// [`agent_script`] entspricht. Ein `grep hum142-stop-1234`, das ein Mensch
/// nebenher tippt, trägt die Marke, aber nicht das Skript — und
/// [`kill_marked`] erschlägt es deshalb nicht. Getroffen werden genau drei:
/// der Sandbox-Prozess, der Shim und der Agent.
///
/// Was **nicht** getroffen wird, sind Kinder des Agenten ohne eigenes Skript
/// (sein `sleep`). Sie brauchen keinen eigenen Blick: Sie leben im
/// PID-Namensraum der Sandbox, dessen Init der Sandbox-Prozess ist. Ist dessen
/// PID weg — und das prüfen beide Tests neben dieser Liste —, hat der Kern den
/// Namensraum abgebaut, und darin kann nichts überlebt haben.
fn marked_processes(marker: &str) -> Vec<u32> {
    let script = agent_script(marker);
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|name| name.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(cmdline) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        if carries_script(&cmdline, &script) {
            found.push(pid);
        }
    }
    found
}

/// Ob diese Kommandozeile das Skript als **eigenes** Argument trägt.
fn carries_script(cmdline: &[u8], script: &str) -> bool {
    cmdline
        .split(|byte| *byte == 0)
        .any(|argument| argument == script.as_bytes())
}

/// Der Vergleich trifft die Prozesse dieses Laufs und keinen fremden.
///
/// Die Marke allein reichte nicht: `kill_marked` schickt `SIGKILL`, und ein
/// `grep` mit der Marke in seinen Argumenten ist ein fremder Prozess auf dem
/// Rechner eines Menschen.
#[test]
fn only_a_whole_argument_counts_as_this_run() {
    let script = agent_script("hum142-probe");
    let agent = format!("/bin/sh\0-c\0{script}\0");

    assert!(
        carries_script(agent.as_bytes(), &script),
        "the agent of this run carries the script as its own argument"
    );
    assert!(
        !carries_script(b"grep\0hum142-probe\0", &script),
        "a grep for the marker is not a process of this run"
    );
    assert!(
        !carries_script(format!("echo\0x{script}y\0").as_bytes(), &script),
        "the script inside a longer argument is not a process of this run"
    );
}

/// Wartet, bis weder der Sandbox-Prozess noch ein markierter Prozess läuft;
/// liefert die Zeit, die es gebraucht hat, sonst `None`.
async fn wait_until_gone(pid: u32, marker: &str, deadline: Duration) -> Option<Duration> {
    let started = std::time::Instant::now();
    while started.elapsed() < deadline {
        if !alive(pid) && marked_processes(marker).is_empty() {
            return Some(started.elapsed());
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    None
}

/// Erschlägt, was von diesem Lauf noch übrig ist.
///
/// Steht vor den Zusicherungen: Ein roter Test soll keinen Prozess auf dem
/// Rechner zurücklassen, und die Messung ist zu diesem Zeitpunkt gemacht.
fn kill_marked(marker: &str) {
    for pid in marked_processes(marker) {
        // SAFETY: `kill` mit einer PID aus `/proc` und einem gewöhnlichen
        // Signal; mehr als `ESRCH` kann dabei nicht herauskommen.
        unsafe {
            libc::kill(i32::try_from(pid).unwrap_or(0), libc::SIGKILL);
        }
    }
}

/// Was aus einem Startversuch geworden ist.
#[derive(Debug)]
enum Started {
    /// Die Sandbox läuft; die PID ihres Prozesses auf dem Wirt.
    Running(u32),
    /// Dieser Rechner kann keine Sandbox starten: kein Sandbox-Backend, eine
    /// zu alte Fassung oder keine unprivilegierten Nutzer-Namensräume
    /// (`SANDBOX_001` bis `SANDBOX_003`). Das ist eine Aussage über die
    /// Maschine und keine über den Abschied.
    Unusable(String),
}

/// Die Befunde, die von der Maschine sprechen und nicht vom Daemon.
const MACHINE_FACTS: &[&str] = &["SANDBOX_001", "SANDBOX_002", "SANDBOX_003"];

/// Liest den Strom eines Starts, bis die Sandbox läuft, und liefert die PID
/// aus der Startzeile.
///
/// Dieselbe Zeile, die der Log-Reiter zeigt: `sandbox <id> started, pid <n>,
/// profile <p>, work dir <d>`.
async fn running_sandbox(events: &mut tonic::Streaming<v1::SandboxEvent>, marker: &str) -> Started {
    let mut pid = None;
    let mut seen: Vec<String> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let next = tokio::time::timeout_at(deadline, events.next()).await;
        let Ok(Some(Ok(event))) = next else {
            panic!("the start ended before the sandbox ran: {next:?}; findings so far: {seen:?}");
        };
        match event.event {
            Some(v1::sandbox_event::Event::Log(line)) => {
                if let Some(found) = pid_from(&line.line) {
                    pid = Some(found);
                }
            }
            Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => {
                seen.push(format!("{} {}", diagnostic.code, diagnostic.why));
            }
            Some(v1::sandbox_event::Event::Status(status))
                if status.state == v1::SandboxState::Failed as i32 =>
            {
                let fact = seen
                    .iter()
                    .find(|text| MACHINE_FACTS.iter().any(|code| text.starts_with(code)));
                return match fact {
                    Some(text) => Started::Unusable(text.clone()),
                    None => {
                        panic!("the sandbox has to start for this test to say anything: {seen:?}")
                    }
                };
            }
            Some(v1::sandbox_event::Event::Status(status))
                if status.state == v1::SandboxState::Running as i32 =>
            {
                assert!(
                    !marked_processes(marker).is_empty(),
                    "the agent of this test is visible on the host"
                );
                return Started::Running(
                    pid.unwrap_or_else(|| panic!("the start line carries the pid: {seen:?}")),
                );
            }
            _ => {}
        }
    }
}

/// Der Zeitstempel der Protokollzeile mit dieser Meldung.
///
/// Das Protokoll ist eine JSON-Zeile je Ereignis; gelesen wird das Feld
/// `timestamp` der ersten Zeile, die die Meldung trägt.
fn stamp_of(log: &str, message: &str) -> Option<chrono::DateTime<chrono::FixedOffset>> {
    let line = log.lines().find(|line| line.contains(message))?;
    let (_, rest) = line.split_once("\"timestamp\":\"")?;
    let (stamp, _) = rest.split_once('"')?;
    chrono::DateTime::parse_from_rfc3339(stamp).ok()
}

/// Die PID aus der Startzeile, wenn die Zeile eine trägt.
fn pid_from(line: &str) -> Option<u32> {
    let (_, rest) = line.split_once("started, pid ")?;
    rest.split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// `Sandbox(Stop)` beendet einen Agenten, der `SIGTERM` abfängt — auch dann,
/// wenn niemand mehr zusieht (HUM-142).
///
/// **Der Strom wird fallen gelassen, und das ist der Punkt.** `humanitl run`
/// schickt an seiner Zeitschranke sein `Sandbox(Stop)` und endet; der
/// Empfänger ist damit weg, bevor der Daemon antworten kann. Bis HUM-142 kehrte
/// `Inner::stop` genau an dieser Stelle zurück — vor dem Töten —, und die
/// Sandbox lief weiter: 8,5 Minuten im einen, über 19 Minuten im anderen
/// gemessenen Fall.
#[tokio::test]
async fn a_stop_ends_an_agent_that_ignores_sigterm_even_without_a_listener() {
    if !shim_is_built() {
        return;
    }
    let mut daemon = Daemon::start(120);
    daemon.ready().await;
    daemon.install_profile();
    let work = daemon.work_dir();
    let marker = format!("hum142-stop-{}", std::process::id());

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();
    let mut events = grpc
        .sandbox(start_request(&work, &marker))
        .await
        .unwrap()
        .into_inner();
    let pid = match running_sandbox(&mut events, &marker).await {
        Started::Running(pid) => pid,
        Started::Unusable(why) => {
            refuse_under_ci(&why);
            return;
        }
    };
    assert!(alive(pid), "the sandbox process {pid} runs");

    // Wie `humanitl run` an seiner Schranke: Stop schicken und gehen.
    let stopping = grpc.sandbox(stop_request()).await.unwrap();
    drop(stopping);
    drop(events);

    let took = wait_until_gone(pid, &marker, STOP_DEADLINE).await;
    let left = marked_processes(&marker);
    let still_alive = alive(pid);
    kill_marked(&marker);
    drop(grpc);
    let ended = daemon.terminate_within(DAEMON_DEADLINE);

    assert!(
        took.is_some(),
        "the stop has a deadline: after {STOP_DEADLINE:?} the sandbox process {pid} is \
         alive={still_alive} and {left:?} still carry the marker"
    );
    eprintln!("hum142: the stop ended the sandbox process {pid} and the agent after {took:?}");
    assert!(
        ended.is_some(),
        "the daemon ends as well once the sandbox is gone"
    );
}

/// Das Ende des Daemons beendet den Agenten, der `SIGTERM` abfängt, und der
/// Daemon selbst endet in beschränkter Zeit (HUM-142).
///
/// Zwei Aussagen in einem Lauf, und beide waren vor diesem Issue falsch: Der
/// Daemon blieb hinter seiner letzten Zeile (`recording flushed`) stehen, weil
/// die Laufzeit beim Fallenlassen ohne Frist auf das blockierende `wait` auf
/// den Prozess der Sandbox wartete, und das Kind hing über `--die-with-parent`
/// an genau diesem Daemon.
#[tokio::test]
async fn the_end_of_the_daemon_ends_the_agent_it_started() {
    if !shim_is_built() {
        return;
    }
    let mut daemon = Daemon::start_logged(120);
    daemon.ready().await;
    daemon.install_profile();
    let work = daemon.work_dir();
    let marker = format!("hum142-shutdown-{}", std::process::id());

    let token = auth::read_token(&daemon.token_path()).unwrap();
    let mut grpc = client::connect_at(&daemon.socket(), &token).await.unwrap();
    let mut events = grpc
        .sandbox(start_request(&work, &marker))
        .await
        .unwrap()
        .into_inner();
    let pid = match running_sandbox(&mut events, &marker).await {
        Started::Running(pid) => pid,
        Started::Unusable(why) => {
            refuse_under_ci(&why);
            return;
        }
    };
    assert!(alive(pid), "the sandbox process {pid} runs");

    // Nur das Signal, kein Warten: Wer hier `wait` sagte, hinge an genau dem
    // Fehler, den dieser Test misst.
    daemon.signal_terminate();
    let signalled = std::time::Instant::now();
    let ended = daemon.wait_within(DAEMON_DEADLINE);
    let daemon_took = signalled.elapsed();
    let gone = wait_until_gone(pid, &marker, STOP_DEADLINE).await;
    let left = marked_processes(&marker);
    let still_alive = alive(pid);
    kill_marked(&marker);

    let log = daemon.log();
    let ended = ended.unwrap_or_else(|| {
        panic!("the daemon has to end within {DAEMON_DEADLINE:?}, sandbox or not: {log}")
    });
    assert!(ended.success(), "SIGTERM is an orderly end: {ended}");
    assert!(
        gone.is_some(),
        "nothing of the sandbox survives the daemon: process {pid} alive={still_alive}, \
         {left:?} still carry the marker"
    );
    // **Die Reihenfolge, nicht nur das Ergebnis.** Dass am Ende kein Prozess
    // übrig ist, sagt für sich genommen wenig: `--die-with-parent` nimmt die
    // Sandbox auch dann mit, wenn der Daemon einfach stirbt. Erst das
    // Protokoll zeigt, ob der Abschied das Kind zuerst beendet hat — dann
    // steht dort das Ende der Sandbox und **kein** `DAEMON_009`, denn keine
    // Aufgabe hing mehr an einem `wait`, das nie zurückkehrt.
    let ended_at = log.find(SANDBOX_ENDED).unwrap_or_else(|| {
        panic!("the farewell ends the sandbox before the daemon goes: {log}");
    });
    assert!(
        !log.contains("DAEMON_009"),
        "with the sandbox gone, no task of this daemon has to be dropped at the deadline: {log}"
    );
    // **Und die beiden Fristen laufen nebeneinander.** Der Dienst lässt offenen
    // Aufrufen `humanitl_ipc::SHUTDOWN_GRACE` (5 s); begänne der Abschied der
    // Sandbox erst danach, addierten sich die Fristen, und ein Mensch wartete
    // auf beide. Die Zeile über das Ende der Sandbox steht deshalb **vor** der
    // Zeile, mit der der Dienst seinen Socket abräumt.
    let served_at = log.find(SERVICE_STOPPED).unwrap_or_else(|| {
        panic!("the service says when it is done: {log}");
    });
    assert!(
        ended_at < served_at,
        "the farewell of the sandbox starts with the signal and not after the last client has \
         gone; instead the sandbox ended at byte {ended_at} and the service at {served_at}: {log}"
    );
    if let (Some(ended_at), Some(served_at)) = (
        stamp_of(&log, SANDBOX_ENDED),
        stamp_of(&log, SERVICE_STOPPED),
    ) {
        eprintln!(
            "hum142: the sandbox ended {} ms before the service had drained its clients",
            (served_at - ended_at).num_milliseconds()
        );
    }
    eprintln!(
        "hum142: the daemon ended as {ended} after {daemon_took:?}, and nothing of the sandbox \
         was left"
    );
    assert!(
        !daemon.socket().exists(),
        "the orderly path removed the socket"
    );
}
