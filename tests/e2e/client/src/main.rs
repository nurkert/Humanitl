//! Der gRPC-Testklient des M1-Demoskripts (HUM-021).
//!
//! Das Skript `tests/e2e/m1_sealed_box.sh` braucht drei Dinge vom Daemon: die
//! Id des ersten wartenden Flows, seine Eckdaten und eine Entscheidung. Die
//! Spezifikation des Issues nennt dafür `humanitl flows list|show|decide`; die
//! Kommandozeile entsteht parallel in HUM-064 und ist noch nicht benutzbar.
//! Bis dahin spricht dieses Binary dieselben RPCs (`ListFlows`, `Decide`,
//! `GetInfo`) über denselben Unix-Socket mit demselben Token, damit das Demo
//! nicht auf ein anderes Issue warten muss.
//!
//! Es ist ausdrücklich ein Testhelfer und kein zweites Produkt: keine
//! Fachlogik, keine eigene Darstellung von Zuständen, nur Verdrahtung. Sobald
//! `humanitl flows decide` steht, nimmt `tests/e2e/lib.sh` die
//! Kommandozeile und dieses Verzeichnis kann verschwinden.
//!
//! ```text
//! e2e-client --socket <daemon.sock> info
//! e2e-client --socket <daemon.sock> wait-held --timeout 10 [--host example.com]
//! e2e-client --socket <daemon.sock> show <FLOW-ID>
//! e2e-client --socket <daemon.sock> decide <FLOW-ID> allow|block [--note TEXT]
//! ```
//!
//! Exit-Codes: `0` erledigt, `1` der Daemon oder die Anfrage hat nein gesagt,
//! `2` die Kommandozeile ist unbrauchbar, `3` eine Wartezeit lief ab.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use humanitl_ipc::{auth, client, v1};

/// Exit-Code für eine unbrauchbare Kommandozeile.
const EXIT_USAGE: u8 = 2;
/// Exit-Code für eine abgelaufene Wartezeit.
const EXIT_TIMEOUT: u8 = 3;

/// Abstand zwischen zwei Abfragen von `ListFlows` beim Warten.
const POLL_INTERVAL: Duration = Duration::from_millis(100);

const USAGE: &str = "\
e2e-client — der gRPC-Testklient des M1-Demos (HUM-021)

usage:
  e2e-client --socket PATH [--token PATH] COMMAND

commands:
  info                          GetInfo als JSON
  wait-held [--timeout SECS] [--host HOST]
                                wartet auf den ersten wartenden Flow und
                                druckt seine Id
  show FLOW-ID                  die Eckdaten eines Flows als JSON
  decide FLOW-ID allow|block [--note TEXT]
                                entscheidet einen wartenden Flow

options:
  --socket PATH   der Socket des Daemons (Pflicht)
  --token PATH    die Token-Datei (Vorgabe: `token` neben dem Socket)
";

/// Was das Binary tun soll.
enum Command {
    /// `GetInfo` als JSON.
    Info,
    /// Auf den ersten wartenden Flow warten.
    WaitHeld {
        /// Wie lange gewartet wird.
        timeout: Duration,
        /// Nur Flows, deren Host diesen Text enthält.
        host: Option<String>,
    },
    /// Die Eckdaten eines Flows als JSON.
    Show {
        /// Der gesuchte Flow.
        flow: String,
    },
    /// Einen wartenden Flow entscheiden.
    Decide {
        /// Der zu entscheidende Flow.
        flow: String,
        /// Erlauben statt blocken.
        allow: bool,
        /// Die Begründung für den Agenten.
        note: Option<String>,
    },
}

/// Die gelesene Kommandozeile.
struct Args {
    /// Der Socket des Daemons.
    socket: PathBuf,
    /// Die Token-Datei.
    token: PathBuf,
    /// Was zu tun ist.
    command: Command,
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(why) => {
            eprintln!("e2e-client: {why}\n{USAGE}");
            return ExitCode::from(EXIT_USAGE);
        }
    };
    match run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Timeout(why)) => {
            eprintln!("e2e-client: {why}");
            ExitCode::from(EXIT_TIMEOUT)
        }
        Err(Failure::Refused(why)) => {
            eprintln!("e2e-client: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Warum der Lauf nicht zu Ende kam.
enum Failure {
    /// Eine Wartezeit lief ab.
    Timeout(String),
    /// Der Daemon oder die Anfrage hat nein gesagt.
    Refused(String),
}

/// Der Lauf selbst.
async fn run(args: Args) -> Result<(), Failure> {
    let token = auth::read_token(&args.token)
        .map_err(|diagnostic| Failure::Refused(format!("{diagnostic}")))?;
    let mut grpc = client::connect_at(&args.socket, &token)
        .await
        .map_err(|diagnostic| Failure::Refused(format!("{diagnostic}")))?;

    match args.command {
        Command::Info => {
            let info = grpc
                .get_info(())
                .await
                .map_err(|status| Failure::Refused(format!("GetInfo: {status}")))?
                .into_inner();
            println!(
                "{}",
                serde_json::json!({
                    "daemon_version": info.daemon_version,
                    "proto_major": info.proto_major,
                    "proto_minor": info.proto_minor,
                    "capabilities": info.capabilities,
                    "session_id": info.session_id,
                })
            );
            Ok(())
        }
        Command::WaitHeld { timeout, host } => {
            let deadline = Instant::now() + timeout;
            loop {
                let page = list(&mut grpc, "state:held").await?;
                let found = page
                    .flows
                    .iter()
                    .find(|summary| host.as_deref().is_none_or(|want| host_of(summary) == want));
                if let Some(summary) = found {
                    println!("{}", summary.flow_id);
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(Failure::Timeout(format!(
                        "no held flow{} within {} seconds",
                        host.map(|host| format!(" for {host}")).unwrap_or_default(),
                        timeout.as_secs()
                    )));
                }
                tokio::time::sleep(POLL_INTERVAL).await;
            }
        }
        Command::Show { flow } => {
            let page = list(&mut grpc, "").await?;
            let Some(summary) = page.flows.iter().find(|row| row.flow_id == flow) else {
                return Err(Failure::Refused(format!("no flow {flow} in the history")));
            };
            println!(
                "{}",
                serde_json::json!({
                    "flow_id": summary.flow_id,
                    "host": host_of(summary),
                    "port": summary.authority.as_ref().map_or(0, |a| a.port),
                    "path": summary.path,
                    "state": state_name(summary.state),
                    "status": summary.status,
                })
            );
            Ok(())
        }
        Command::Decide { flow, allow, note } => {
            let decision = if allow {
                v1::decide_request::Decision::Allow(())
            } else {
                v1::decide_request::Decision::Block(v1::decide_request::Block {
                    note: note.unwrap_or_default(),
                })
            };
            let response = grpc
                .decide(v1::DecideRequest {
                    flow_ids: vec![flow.clone()],
                    decision: Some(decision),
                    ..v1::DecideRequest::default()
                })
                .await
                .map_err(|status| Failure::Refused(format!("Decide: {status}")))?
                .into_inner();
            let applied = response
                .results
                .first()
                .is_some_and(|result| result.applied);
            if applied {
                Ok(())
            } else {
                Err(Failure::Refused(format!(
                    "the daemon did not decide {flow}"
                )))
            }
        }
    }
}

/// Eine Seite der Historie, gefiltert wie im History-Screen.
async fn list(grpc: &mut client::Client, filter: &str) -> Result<v1::FlowPage, Failure> {
    Ok(grpc
        .list_flows(v1::ListFlowsRequest {
            filter: filter.to_owned(),
            limit: 1000,
            ..v1::ListFlowsRequest::default()
        })
        .await
        .map_err(|status| Failure::Refused(format!("ListFlows: {status}")))?
        .into_inner())
}

/// Der Host eines Flows, leer wenn die Anfrage keine Authority trug.
fn host_of(summary: &v1::FlowSummary) -> &str {
    summary
        .authority
        .as_ref()
        .map_or("", |authority| authority.host.as_str())
}

/// Der Kurzname eines Zustands, wie ihn auch der Filter erwartet.
fn state_name(state: i32) -> String {
    v1::FlowState::try_from(state)
        .unwrap_or(v1::FlowState::Unspecified)
        .as_str_name()
        .trim_start_matches("FLOW_STATE_")
        .to_ascii_lowercase()
}

impl Args {
    /// Liest die Kommandozeile.
    fn parse(args: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut socket: Option<PathBuf> = None;
        let mut token: Option<PathBuf> = None;
        let mut rest: Vec<String> = Vec::new();
        let mut timeout = Duration::from_secs(10);
        let mut host: Option<String> = None;
        let mut note: Option<String> = None;

        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--socket" => socket = Some(PathBuf::from(next(&mut args, "--socket")?)),
                "--token" => token = Some(PathBuf::from(next(&mut args, "--token")?)),
                "--timeout" => {
                    let secs = next(&mut args, "--timeout")?;
                    let secs: u64 = secs
                        .parse()
                        .map_err(|_err| format!("--timeout wants whole seconds, got {secs:?}"))?;
                    timeout = Duration::from_secs(secs);
                }
                "--host" => host = Some(next(&mut args, "--host")?),
                "--note" => note = Some(next(&mut args, "--note")?),
                "-h" | "--help" => return Err("usage".to_owned()),
                other if other.starts_with("--") => {
                    return Err(format!("unknown option {other}"));
                }
                other => rest.push(other.to_owned()),
            }
        }

        let Some(socket) = socket else {
            return Err("--socket is required".to_owned());
        };
        let token = token.unwrap_or_else(|| token_beside(&socket));
        let Some(name) = rest.first().map(String::as_str) else {
            return Err("no command".to_owned());
        };
        let command = match (name, rest.get(1), rest.get(2)) {
            ("info", None, _) => Command::Info,
            ("wait-held", None, _) => Command::WaitHeld { timeout, host },
            ("show", Some(flow), None) => Command::Show { flow: flow.clone() },
            ("decide", Some(flow), Some(verdict)) => Command::Decide {
                flow: flow.clone(),
                allow: match verdict.as_str() {
                    "allow" => true,
                    "block" => false,
                    other => return Err(format!("decide wants allow or block, got {other:?}")),
                },
                note,
            },
            (other, _, _) => return Err(format!("unknown command {other:?}")),
        };
        Ok(Self {
            socket,
            token,
            command,
        })
    }
}

/// Das nächste Argument einer Option, oder ein Fehler mit ihrem Namen.
fn next(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} wants a value"))
}

/// Die Token-Datei neben einem Socket.
fn token_beside(socket: &Path) -> PathBuf {
    socket
        .parent()
        .map_or_else(|| PathBuf::from("token"), |dir| dir.join("token"))
}
