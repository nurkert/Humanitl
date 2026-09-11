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
        let dir = tempfile::Builder::new()
            .prefix("hum")
            .tempdir_in("/tmp")
            .expect("a short temporary directory for sun_path");
        for name in ["run", "data", "config", "home"] {
            std::fs::create_dir(dir.path().join(name)).unwrap();
        }
        let child = spawn(dir.path(), hold_timeout_secs, llm_endpoint);
        Self { dir, child }
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
fn spawn(dir: &Path, hold_timeout_secs: u64, llm_endpoint: Option<&str>) -> Child {
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

/// Ein XDG-Baum mit einer `config.toml`, die `resolver.test_ca` setzt.
///
/// Über die Datei und nicht über eine Umgebungsvariable: Genau diesen Weg
/// nimmt eine Konfiguration im Alltag, und genau er darf das Vertrauen nicht
/// allein herstellen.
fn tree_with_test_ca(test_ca: &Path) -> tempfile::TempDir {
    let dir = tempfile::Builder::new()
        .prefix("hum")
        .tempdir_in("/tmp")
        .expect("a short temporary directory for sun_path");
    for name in ["run", "data", "config", "home"] {
        std::fs::create_dir(dir.path().join(name)).unwrap();
    }
    let config = dir.path().join("config").join("humanitl");
    std::fs::create_dir_all(&config).unwrap();
    std::fs::write(
        config.join("config.toml"),
        format!("[resolver]\ntest_ca = \"{}\"\n", test_ca.display()),
    )
    .unwrap();
    dir
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
