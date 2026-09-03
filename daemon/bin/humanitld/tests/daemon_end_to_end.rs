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

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
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
        let dir = tempfile::Builder::new()
            .prefix("hum")
            .tempdir_in("/tmp")
            .expect("a short temporary directory for sun_path");
        for name in ["run", "data", "config", "home"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        let child = spawn(dir.path(), hold_timeout_secs);
        Self { dir, child }
    }

    /// Beendet den Daemon und startet einen neuen im selben Baum.
    ///
    /// Der zweite Prozess hat eine leere Registry und eine neue Sitzung; was er
    /// über frühere Flows sagt, kann deshalb nur aus der Aufzeichnung kommen.
    async fn restart(&mut self, hold_timeout_secs: u64) {
        self.terminate();
        self.child = spawn(self.dir.path(), hold_timeout_secs);
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
        // SIGTERM statt `Child::kill` (`SIGKILL`): nur der geordnete Weg räumt
        // Socket und Token weg, und genau das soll hier geprüft werden.
        let pid = i32::try_from(self.child.id()).unwrap();
        // SAFETY: `kill` mit einer eigenen, noch nicht abgeernteten Kind-PID.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        let status = self.child.wait().expect("the daemon must be reapable");
        assert!(status.success(), "SIGTERM is an orderly end: {status}");
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Startet das gebaute Binary in diesem XDG-Baum.
fn spawn(dir: &Path, hold_timeout_secs: u64) -> Child {
    Command::new(env!("CARGO_BIN_EXE_humanitld"))
        .env("XDG_RUNTIME_DIR", dir.join("run"))
        .env("XDG_DATA_HOME", dir.join("data"))
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("HOME", dir.join("home"))
        .env("HUMANITL_HOLD__TIMEOUT_SECS", hold_timeout_secs.to_string())
        .spawn()
        .expect("the daemon binary must start")
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
