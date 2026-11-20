//! `humanitl agents status|source|remove|install|update|login|callback|config`
//! (HUM-518, HUM-563, HUM-520, ADR-0023).
//!
//! Ein gRPC-Aufruf je Befehl und sonst nichts (ADR-018): welche Quelle gilt,
//! ob eine Firmenvorgabe sie sperrt, welche Version im Depot liegt und ob eine
//! Sitzung es nutzt, weiß der Daemon. Die Befehle nennen keinen Agenten
//! selbst; sie zeigen, was der Daemon je registriertem Agenten meldet.
//!
//! `install` und `update` starten im Daemon einen Wartungslauf (eine Sandbox
//! ohne Projekt), hängen dieses Terminal an dessen Terminal und fragen nach
//! dem Ende, was der Daemon übernommen hat. Auch das ist kein Fachwissen des
//! Clients: Version, Hosts und Prüfung kennt nur der Daemon.
//!
//! `login` und `config` (HUM-520) sind Wartungsläufe derselben Art. Welche
//! URL einer Anmeldung sich öffnen lässt, sagt der Daemon (`openable`); die
//! CLI öffnet nur mit `--open` und nur eine solche. Welche Schlüssel einer
//! Konfiguration Befehle ausführen, sagt ebenfalls der Daemon; die CLI hebt
//! sie hervor und fragt vor der Übernahme.

use std::io::{BufRead as _, IsTerminal as _};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cli::AgentsCmd;
use crate::cmd::attach::{Ending, attach_sandbox, resolve_detach_keys};
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Wie oft nach dem Ende des Laufs nach seinem Ergebnis gefragt wird.
const OUTCOME_POLL: Duration = Duration::from_millis(250);

/// Wie lange höchstens auf das Ergebnis gewartet wird: Die Übernahme prüft
/// und kopiert bis zu 2 GiB.
const OUTCOME_WAIT: Duration = Duration::from_secs(600);

/// `running`, solange der Lauf geht (`AgentMaintenance.state`).
const RUNNING: &str = "running";
/// `done` nach der Übernahme.
const DONE: &str = "done";
/// `confirm`: ein Konfigurationslauf wartet auf die Bestätigung (HUM-520).
const CONFIRM: &str = "confirm";

/// Führt `humanitl agents <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet; der Befund des Daemons, wenn er
/// ablehnt (`AGENT_010` unter einer Sperre, `AGENT_018` für ein Depot in
/// Gebrauch); `CLI_004`, wenn `remove` ohne Terminal und ohne `--yes` nicht
/// fragen kann oder die Antwort nicht ja war.
pub async fn run(ctx: &Context, cmd: &AgentsCmd) -> Result<u8, Failure> {
    let (op, only) = match cmd {
        AgentsCmd::Status { agent } => (
            v1::agents_request::Op::Status(v1::agents_request::Status {}),
            agent.clone(),
        ),
        AgentsCmd::Source { agent, source } => (
            v1::agents_request::Op::SetSource(v1::agents_request::SetSource {
                agent: agent.clone(),
                source: source.clone(),
            }),
            Some(agent.clone()),
        ),
        AgentsCmd::Install {
            agent,
            version,
            yes,
        } => {
            return maintain(ctx, Task::Install, agent, version.as_deref(), *yes).await;
        }
        AgentsCmd::Update {
            agent,
            version,
            yes,
        } => {
            return maintain(ctx, Task::Update, agent, version.as_deref(), *yes).await;
        }
        AgentsCmd::Login { agent, open, yes } => {
            return maintain(ctx, Task::Login { open: *open }, agent, None, *yes).await;
        }
        AgentsCmd::Callback { agent, url } => return callback(ctx, agent, url).await,
        AgentsCmd::Config {
            agent,
            show,
            accept,
            discard,
            yes,
        } => {
            if *show {
                return show_config(ctx, agent).await;
            }
            if *accept || *discard {
                return decide_config(ctx, agent, *accept).await;
            }
            return maintain(ctx, Task::Config, agent, None, *yes).await;
        }
        AgentsCmd::Remove { agent, yes } => {
            if !yes {
                confirm(ctx, agent)?;
            }
            (
                v1::agents_request::Op::Remove(v1::agents_request::Remove {
                    agent: agent.clone(),
                }),
                Some(agent.clone()),
            )
        }
    };
    let mut client = ctx.connect().await?;
    let status = client
        .agents(v1::AgentsRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
        .into_inner();
    let shown: Vec<&v1::AgentDepot> = status
        .agents
        .iter()
        .filter(|entry| only.as_deref().is_none_or(|agent| entry.agent == agent))
        .collect();
    if let Some(agent) = only.as_deref()
        && shown.is_empty()
    {
        return Err(Failure::new(
            Diagnostic::builder(codes::CONFIG_003, Severity::Error)
                .why(format!("{agent:?} is no agent of the depot"))
                .fix(FixAction::CopyCommand("humanitl agents status".to_owned()))
                .build(),
        ));
    }
    print(ctx, &shown, status.encryption.as_ref());
    Ok(EXIT_OK)
}

/// Installation, Update, Anmeldung oder Konfigurationslauf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Task {
    Install,
    Update,
    /// `open`: die erste URL zu einem Anmeldehost des Adapters öffnen.
    Login {
        open: bool,
    },
    Config,
}

impl Task {
    const fn word(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
            Self::Login { .. } => "login",
            Self::Config => "config",
        }
    }

    /// Die Frage vor dem Lauf.
    fn question(self, agent: &str) -> String {
        match self {
            Self::Install | Self::Update => format!(
                "{} {agent} in the depot? It runs in a sandbox without your project; only the \
                 vendor's hosts pass without a hold, and nothing downloaded runs on this machine. \
                 [y/N] ",
                if self == Self::Install {
                    "Install"
                } else {
                    "Update"
                }
            ),
            Self::Login { .. } => format!(
                "Log {agent} in? Its depot program runs in a sandbox without your project; only \
                 its login hosts pass without a hold, and only its checked credential file goes \
                 back into the depot. [y/N] "
            ),
            Self::Config => format!(
                "Run {agent} on a copy of its configuration? It runs in a sandbox without your \
                 project; afterwards you see every changed file and decide whether it is taken \
                 over. [y/N] "
            ),
        }
    }
}

/// `humanitl agents install|update`: startet den Lauf, hängt sich an sein
/// Terminal und meldet, was übernommen wurde.
async fn maintain(
    ctx: &Context,
    task: Task,
    agent: &str,
    version: Option<&str>,
    yes: bool,
) -> Result<u8, Failure> {
    if !yes {
        ask(
            ctx,
            &task.question(agent),
            &format!("humanitl agents {} {agent} --yes", task.word()),
            &format!("nothing was started; the answer was not yes, so the depot of {agent} stays"),
        )?;
    }
    // Vor der Verbindung: Ein unlesbares Präfix soll nicht erst nach dem
    // Start des Laufs auffallen.
    let keys = resolve_detach_keys(ctx, None)?;
    let wish = v1::agents_request::Maintain {
        agent: agent.to_owned(),
        version: version.unwrap_or_default().to_owned(),
    };
    let op = match task {
        Task::Install => v1::agents_request::Op::Install(wish),
        Task::Update => v1::agents_request::Op::Update(wish),
        Task::Login { .. } => v1::agents_request::Op::Login(wish),
        Task::Config => v1::agents_request::Op::Config(wish),
    };
    let mut client = ctx.connect().await?;
    let status = client
        .agents(v1::AgentsRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
        .into_inner();
    let started = maintenance_of(&status, agent)?;
    if started.state == RUNNING && !started.sandbox_id.is_empty() {
        let opener = match task {
            Task::Login { open: true } => Some(tokio::spawn(open_login_url(
                ctx.connect().await?,
                agent.to_owned(),
                started.sandbox_id.clone(),
            ))),
            _ => None,
        };
        let finished = attended(ctx, agent, &started, keys).await;
        if let Some(opener) = opener {
            opener.abort();
        }
        return match finished? {
            Attended::Ended(outcome) => finish(ctx, task, agent, &outcome, yes).await,
            Attended::Detached => Ok(EXIT_OK),
            Attended::Signal(number) => Ok(128_u8.saturating_add(number)),
        };
    }
    finish(ctx, task, agent, &started, yes).await
}

/// Wie ein Lauf am Terminal ausging.
enum Attended {
    /// Er ist vorbei, mit diesem Ergebnis.
    Ended(v1::AgentMaintenance),
    /// Der Mensch hat sich abgehängt; der Lauf geht weiter.
    Detached,
    /// Ein Signal beendete das Anhängen.
    Signal(u8),
}

/// Hängt sich an das Terminal des Laufs und wartet auf sein Ergebnis.
async fn attended(
    ctx: &Context,
    agent: &str,
    started: &v1::AgentMaintenance,
    keys: humanitl_config::DetachKeys,
) -> Result<Attended, Failure> {
    if !ctx.render.is_json() {
        match attach_or_outcome(ctx, agent, &started.sandbox_id, keys).await? {
            Attached::Ended(outcome) => return Ok(Attended::Ended(outcome)),
            Attached::Terminal(Ending::Stream(_)) => {}
            Attached::Terminal(Ending::Detached) => {
                ctx.render.note(&format!(
                    "Detached; the run keeps going. Its result: humanitl agents status {agent}"
                ));
                return Ok(Attended::Detached);
            }
            Attached::Terminal(Ending::Signal(number)) => {
                return Ok(Attended::Signal(number));
            }
        }
    }
    let outcome = wait_for_outcome(ctx, agent, &started.sandbox_id).await?;
    Ok(Attended::Ended(outcome))
}

/// Nach dem Lauf: das Ergebnis, und für einen Konfigurationslauf mit
/// Änderungen die Frage, ob sie übernommen werden.
async fn finish(
    ctx: &Context,
    task: Task,
    agent: &str,
    outcome: &v1::AgentMaintenance,
    yes: bool,
) -> Result<u8, Failure> {
    if task == Task::Config && outcome.state == CONFIRM {
        print_changes(ctx, agent, &outcome.config_changes, "changed");
        if ctx.render.is_json() && !yes {
            return Ok(EXIT_OK);
        }
        let take = yes
            || ask(
                ctx,
                &format!("Take these changes into the configuration of {agent}? [y/N] "),
                &format!("humanitl agents config {agent} --accept"),
                &format!(
                    "nothing was taken over; the changes wait: humanitl agents config {agent} \
                     --accept or --discard"
                ),
            )
            .is_ok();
        return decide_config(ctx, agent, take).await;
    }
    report(ctx, agent, outcome)
}

/// `humanitl agents config <id> --accept|--discard`: entscheidet über einen
/// wartenden Konfigurationslauf.
async fn decide_config(ctx: &Context, agent: &str, accept: bool) -> Result<u8, Failure> {
    let wish = v1::agents_request::Agent {
        agent: agent.to_owned(),
    };
    let op = if accept {
        v1::agents_request::Op::ConfigAccept(wish)
    } else {
        v1::agents_request::Op::ConfigDiscard(wish)
    };
    let mut client = ctx.connect().await?;
    client
        .agents(v1::AgentsRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?;
    if ctx.render.is_json() {
        ctx.render
            .value(&json!({ "agent": agent, "accepted": accept }));
    } else if accept {
        ctx.render.line(&format!(
            "{agent}: the changed configuration is in the depot now"
        ));
    } else {
        ctx.render.line(&format!(
            "{agent}: the changes were discarded; the configuration stays as it was"
        ));
    }
    Ok(EXIT_OK)
}

/// `humanitl agents config <id> --show`.
async fn show_config(ctx: &Context, agent: &str) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let status = client
        .agents(v1::AgentsRequest {
            op: Some(v1::agents_request::Op::ConfigShow(
                v1::agents_request::Agent {
                    agent: agent.to_owned(),
                },
            )),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
        .into_inner();
    let files = status
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .map(|entry| entry.config_files.clone())
        .unwrap_or_default();
    print_changes(ctx, agent, &files, "file");
    Ok(EXIT_OK)
}

/// Die Dateien einer Konfiguration, Schlüssel mit Befehlen hervorgehoben.
fn print_changes(ctx: &Context, agent: &str, changes: &[v1::AgentConfigChange], what: &str) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "agent": agent,
            "files": changes.iter().map(|change| json!({
                "path": change.path,
                "change": change.change,
                "keys": change.keys,
                "command_keys": change.command_keys,
                "keys_omitted": change.keys_omitted,
            })).collect::<Vec<_>>(),
        }));
        return;
    }
    if changes.is_empty() {
        ctx.render.line(&format!("{agent}: no configuration file"));
        return;
    }
    for change in changes {
        let label = if what == "file" {
            String::new()
        } else {
            format!(" ({})", change.change)
        };
        ctx.render.line(&format!("{}{label}", change.path));
        for key in &change.keys {
            if change.command_keys.contains(key) {
                ctx.render.line(&format!(
                    "  ! {key}    runs commands, read it before you agree"
                ));
            } else {
                ctx.render.line(&format!("    {key}"));
            }
        }
        if change.keys_omitted > 0 {
            ctx.render.line(&format!(
                "    ... and {} more changed keys, none of them runs commands",
                change.keys_omitted
            ));
        }
    }
}

/// `humanitl agents callback <id> <url>`.
async fn callback(ctx: &Context, agent: &str, url: &str) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    client
        .agents(v1::AgentsRequest {
            op: Some(v1::agents_request::Op::DeliverCallback(
                v1::agents_request::Callback {
                    agent: agent.to_owned(),
                    url: url.to_owned(),
                },
            )),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?;
    if ctx.render.is_json() {
        ctx.render
            .value(&json!({ "agent": agent, "delivered": true }));
    } else {
        ctx.render.line(&format!(
            "{agent}: the callback was delivered inside the login run"
        ));
    }
    Ok(EXIT_OK)
}

/// Öffnet mit `--open` die erste URL eines Anmeldelaufs, die der Daemon als
/// öffenbar meldet (ein Anmeldehost des Adapters), genau einmal im Browser
/// dieses Rechners. Jede andere URL bleibt ungeöffnet.
async fn open_login_url(mut client: humanitl_ipc::client::Client, agent: String, sandbox: String) {
    loop {
        tokio::time::sleep(OUTCOME_POLL).await;
        let Ok(status) = client
            .agents(v1::AgentsRequest {
                op: Some(v1::agents_request::Op::Status(
                    v1::agents_request::Status {},
                )),
            })
            .await
        else {
            return;
        };
        let Ok(run) = maintenance_of(&status.into_inner(), &agent) else {
            return;
        };
        if run.sandbox_id != sandbox || run.state != RUNNING {
            return;
        }
        if let Some(url) = run.login_urls.iter().find(|url| url.openable) {
            open_on_host(&url.url);
            return;
        }
    }
}

/// Der Browser des Rechners für eine geprüfte URL: ein Argument, keine Shell.
fn open_on_host(url: &str) {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let _ = std::process::Command::new(opener)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Wie das Anhängen ausging: am Terminal, oder der Lauf war schon vorbei.
enum Attached {
    Terminal(Ending),
    Ended(v1::AgentMaintenance),
}

/// Hängt sich an das Terminal des Laufs. Scheitert das, fragt die CLI genau
/// einmal nach dem Lauf: Ist genau dieser Lauf (dieselbe Sandbox) schon
/// vorbei, zählt sein Ergebnis, denn dann gab es kein Terminal mehr. In jedem
/// anderen Fall, auch wenn der Lauf noch läuft und nur die Verbindung zum
/// Terminal brach, bleibt der Befund des Anhängens.
async fn attach_or_outcome(
    ctx: &Context,
    agent: &str,
    sandbox: &str,
    keys: humanitl_config::DetachKeys,
) -> Result<Attached, Failure> {
    let read_only = !std::io::stdin().is_terminal();
    match attach_sandbox(ctx, read_only, keys, sandbox).await {
        Ok(ending) => Ok(Attached::Terminal(ending)),
        Err(failure) => match last_run(ctx, agent).await {
            Ok(run) if run.sandbox_id == sandbox && run.state != RUNNING => {
                Ok(Attached::Ended(run))
            }
            _ => Err(failure),
        },
    }
}

/// Der letzte Lauf von `agent`, wie der Daemon ihn gerade nennt.
async fn last_run(ctx: &Context, agent: &str) -> Result<v1::AgentMaintenance, Failure> {
    let mut client = ctx.connect().await?;
    let status = client
        .agents(v1::AgentsRequest {
            op: Some(v1::agents_request::Op::Status(
                v1::agents_request::Status {},
            )),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
        .into_inner();
    maintenance_of(&status, agent)
}

/// Der Lauf von `agent` aus einer Antwort.
fn maintenance_of(status: &v1::AgentsStatus, agent: &str) -> Result<v1::AgentMaintenance, Failure> {
    status
        .agents
        .iter()
        .find(|entry| entry.agent == agent)
        .and_then(|entry| entry.maintenance.clone())
        .ok_or_else(|| {
            Failure::new(
                Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(format!(
                        "the daemon answered without a maintenance run of {agent}; it may be \
                         older than this command (protocol 1.52)"
                    ))
                    .build(),
            )
        })
}

/// Fragt nach dem Ende des Laufs, bis der Daemon sein Ergebnis nennt.
async fn wait_for_outcome(
    ctx: &Context,
    agent: &str,
    sandbox: &str,
) -> Result<v1::AgentMaintenance, Failure> {
    let mut client = ctx.connect().await?;
    let deadline = Instant::now() + OUTCOME_WAIT;
    loop {
        let status = client
            .agents(v1::AgentsRequest {
                op: Some(v1::agents_request::Op::Status(
                    v1::agents_request::Status {},
                )),
            })
            .await
            .map_err(|status| Failure::new(status_diagnostic(&status, "Agents")))?
            .into_inner();
        let outcome = maintenance_of(&status, agent)?;
        if outcome.sandbox_id != sandbox {
            // Ein anderer Lauf hat diesen abgelöst; sein Ergebnis ist nicht
            // das dieses Befehls.
            return Err(Failure::new(
                Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(format!(
                        "the maintenance run of {agent} in sandbox {sandbox} was followed by \
                         another run ({}) before its result could be read",
                        outcome.sandbox_id
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "humanitl agents status {agent}"
                    )))
                    .build(),
            ));
        }
        if outcome.state != RUNNING {
            return Ok(outcome);
        }
        if Instant::now() >= deadline {
            return Err(Failure::new(
                Diagnostic::builder(codes::IPC_006, Severity::Error)
                    .why(format!(
                        "the maintenance run of {agent} ended, and the daemon named no result \
                         within {} s",
                        OUTCOME_WAIT.as_secs()
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "humanitl agents status {agent}"
                    )))
                    .build(),
            ));
        }
        tokio::time::sleep(OUTCOME_POLL).await;
    }
}

/// Sagt, was der Lauf übernommen hat, oder warum nichts.
fn report(ctx: &Context, agent: &str, outcome: &v1::AgentMaintenance) -> Result<u8, Failure> {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "agent": agent,
            "sandbox_id": outcome.sandbox_id,
            "task": outcome.task,
            "state": outcome.state,
            "version": outcome.version,
            "previous": outcome.previous,
            "diagnostics": outcome.diagnostics.iter().map(|found| json!({
                "code": found.code,
                "why": found.why,
            })).collect::<Vec<_>>(),
        }));
    }
    let mut found: Vec<Diagnostic> = outcome.diagnostics.iter().filter_map(from_proto).collect();
    if outcome.state == DONE {
        if !ctx.render.is_json() {
            ctx.render.line(&outcome_line(agent, outcome));
            for diagnostic in &found {
                ctx.render.diagnostic(diagnostic);
            }
        }
        return Ok(EXIT_OK);
    }
    let first = if found.is_empty() {
        Diagnostic::builder(codes::AGENT_011, Severity::Error)
            .why(format!(
                "the maintenance run of {agent} ended without a result; nothing was taken over"
            ))
            .build()
    } else {
        found.remove(0)
    };
    if !ctx.render.is_json() {
        for diagnostic in &found {
            ctx.render.diagnostic(diagnostic);
        }
    }
    Err(Failure::new(first))
}

/// Die Zeile nach einem geglückten Lauf.
fn outcome_line(agent: &str, outcome: &v1::AgentMaintenance) -> String {
    if outcome.task == "login" {
        return format!(
            "{agent}: logged in; the depot keeps the checked credential file, and every session \
             starts with it"
        );
    }
    if outcome.task == "config" {
        return format!("{agent}: the run changed nothing in the configuration");
    }
    if outcome.previous.is_empty() {
        format!(
            "{agent}: version {} installed in the depot",
            outcome.version
        )
    } else if outcome.previous == outcome.version {
        format!(
            "{agent}: version {} is the current one; nothing changed",
            outcome.version
        )
    } else {
        format!(
            "{agent}: version {} is current now, before {}; running sessions keep theirs",
            outcome.version, outcome.previous
        )
    }
}

/// Fragt vor `remove` auf dem Terminal nach; ohne Terminal ist `--yes` nötig.
fn confirm(ctx: &Context, agent: &str) -> Result<(), Failure> {
    use std::io::Write as _;

    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "removing the depot of {agent} deletes its program, configuration and login \
                     and cannot be undone, so it asks first, and there is no terminal to ask on"
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents remove {agent} --yes"
                )))
                .build(),
        ));
    }
    let mut stderr = std::io::stderr();
    let _ = write!(
        stderr,
        "Remove the depot of {agent} with its program, configuration and login? [y/N] "
    );
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = stdin.lock().read_line(&mut answer);
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Info)
            .why(format!(
                "nothing was removed; the answer was not yes, so the depot of {agent} stays"
            ))
            .build(),
    ))
}

/// Fragt vor `install` und `update` auf dem Terminal nach; ohne Terminal ist
/// `--yes` nötig.
fn ask(ctx: &Context, question: &str, with_yes: &str, declined: &str) -> Result<(), Failure> {
    use std::io::Write as _;

    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(
                    "a maintenance run starts a sandbox that downloads an agent into the depot, \
                     so it asks first, and there is no terminal to ask on"
                        .to_owned(),
                )
                .fix(FixAction::CopyCommand(with_yes.to_owned()))
                .build(),
        ));
    }
    let mut stderr = std::io::stderr();
    let _ = write!(stderr, "{question}");
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = stdin.lock().read_line(&mut answer);
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Info)
            .why(declined.to_owned())
            .build(),
    ))
}

fn print(ctx: &Context, shown: &[&v1::AgentDepot], encryption: Option<&v1::CredentialEncryption>) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "encryption": encryption.map(|state| json!({
                "enabled": state.enabled,
                "configured": state.configured,
                "keyring": state.keyring,
                "diagnostics": state.diagnostics.iter().map(|found| json!({
                    "code": found.code,
                    "why": found.why,
                })).collect::<Vec<_>>(),
            })),
            "agents": shown.iter().map(|entry| json!({
                "agent": entry.agent,
                "name": entry.display_name,
                "source": entry.source,
                "origin": entry.origin,
                "locked_by": entry.locked_by,
                "depot_supported": entry.depot_supported,
                "version": entry.version,
                "signature": entry.signature,
                "logged_in": entry.logged_in,
                "credentials_encrypted": entry.credentials_encrypted,
                "last_writeback": entry.last_writeback,
                "size_bytes": entry.size_bytes,
                "in_use": entry.in_use,
                "diagnostics": entry.diagnostics.iter().map(|found| json!({
                    "code": found.code,
                    "why": found.why,
                })).collect::<Vec<_>>(),
                "maintenance": entry.maintenance.as_ref().map(maintenance_json),
            })).collect::<Vec<_>>(),
        }));
        return;
    }
    // Ein Daemon vor 1.56 schickt keinen Stand der Verschlüsselung (HUM-523).
    if let Some(state) = encryption {
        ctx.render.line(&encryption_line(state));
        for diagnostic in state.diagnostics.iter().filter_map(from_proto) {
            ctx.render.diagnostic(&diagnostic);
        }
    }
    for entry in shown {
        ctx.render.line(&line(entry));
        for diagnostic in entry.diagnostics.iter().filter_map(from_proto) {
            ctx.render.diagnostic(&diagnostic);
        }
        if let Some(run) = &entry.maintenance {
            ctx.render.line(&maintenance_line(run));
            for diagnostic in run.diagnostics.iter().filter_map(from_proto) {
                ctx.render.diagnostic(&diagnostic);
            }
        }
    }
}

/// Der letzte Wartungslauf als JSON (HUM-563).
fn maintenance_json(run: &v1::AgentMaintenance) -> serde_json::Value {
    json!({
        "sandbox_id": run.sandbox_id,
        "task": run.task,
        "state": run.state,
        "version": run.version,
        "previous": run.previous,
        "login_urls": run.login_urls.iter().map(|url| json!({
            "url": url.url,
            "openable": url.openable,
        })).collect::<Vec<_>>(),
        "callback_paste": run.callback_paste,
        "diagnostics": run.diagnostics.iter().map(|found| json!({
            "code": found.code,
            "why": found.why,
        })).collect::<Vec<_>>(),
    })
}

/// Die Zeile zum letzten Wartungslauf eines Agenten (HUM-563).
fn maintenance_line(run: &v1::AgentMaintenance) -> String {
    let sandbox = if run.sandbox_id.is_empty() {
        "no sandbox".to_owned()
    } else {
        format!("sandbox {}", run.sandbox_id)
    };
    let what = match run.state.as_str() {
        RUNNING => "running".to_owned(),
        DONE if run.version.is_empty() => "done".to_owned(),
        DONE => format!("done, version {}", run.version),
        CONFIRM => "waiting for your confirmation".to_owned(),
        other => other.to_owned(),
    };
    format!("  last {}: {what} ({sandbox})", run.task)
}

/// Die Zeile zur Verschlüsselung der Zugangsdateien (HUM-523), so wie das
/// Depot sie auf der Platte sagt.
fn encryption_line(state: &v1::CredentialEncryption) -> String {
    if state.enabled {
        "Credential files: encrypted, the key in the system keyring \
         (agents.encrypt_credentials = true)"
            .to_owned()
    } else {
        "Credential files: not encrypted (agents.encrypt_credentials = false)".to_owned()
    }
}

/// Eine Zeile je Agent: Quelle mit Herkunft oder Sperre, dann das Depot.
fn line(entry: &v1::AgentDepot) -> String {
    let origin = if entry.locked_by.is_empty() {
        entry.origin.clone()
    } else {
        format!("fixed by your company: {}", entry.locked_by)
    };
    let depot = if entry.version.is_empty() {
        "not installed".to_owned()
    } else {
        format!(
            "{} ({}), {}{}",
            entry.version,
            if entry.signature.is_empty() {
                "signature unknown"
            } else {
                entry.signature.as_str()
            },
            match (entry.logged_in, entry.credentials_encrypted) {
                (true, true) => "logged in, encrypted",
                (true, false) => "logged in",
                (false, _) => "not logged in",
            },
            if entry.in_use { ", in use" } else { "" }
        )
    };
    format!(
        "{} ({}): source {} ({origin}); depot {depot}; {} bytes",
        entry.display_name, entry.agent, entry.source, entry.size_bytes
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_locked_source_names_the_policy() {
        let entry = v1::AgentDepot {
            agent: "claude".to_owned(),
            display_name: "Claude Code".to_owned(),
            source: "depot".to_owned(),
            origin: "policy".to_owned(),
            locked_by: "/etc/humanitl/policy.toml".to_owned(),
            version: "2.1.0".to_owned(),
            signature: "verified".to_owned(),
            ..v1::AgentDepot::default()
        };
        assert_eq!(
            line(&entry),
            "Claude Code (claude): source depot (fixed by your company: \
             /etc/humanitl/policy.toml); depot 2.1.0 (verified), not logged in; 0 bytes"
        );
    }

    /// HUM-523: the status says "encrypted" for the depot and per agent.
    #[test]
    fn the_status_says_encrypted() {
        let entry = v1::AgentDepot {
            agent: "claude".to_owned(),
            display_name: "Claude Code".to_owned(),
            source: "depot".to_owned(),
            origin: "agent".to_owned(),
            version: "2.1.0".to_owned(),
            signature: "verified".to_owned(),
            logged_in: true,
            credentials_encrypted: true,
            ..v1::AgentDepot::default()
        };
        assert_eq!(
            line(&entry),
            "Claude Code (claude): source depot (agent); depot 2.1.0 (verified), logged in, \
             encrypted; 0 bytes"
        );
        let on = v1::CredentialEncryption {
            enabled: true,
            ..v1::CredentialEncryption::default()
        };
        assert!(encryption_line(&on).starts_with("Credential files: encrypted"));
        let off = v1::CredentialEncryption::default();
        assert!(encryption_line(&off).starts_with("Credential files: not encrypted"));
    }

    #[test]
    fn the_outcome_line_says_what_changed() {
        let run = |version: &str, previous: &str| v1::AgentMaintenance {
            state: DONE.to_owned(),
            version: version.to_owned(),
            previous: previous.to_owned(),
            ..v1::AgentMaintenance::default()
        };
        assert_eq!(
            outcome_line("claude", &run("2.1.89", "")),
            "claude: version 2.1.89 installed in the depot"
        );
        assert_eq!(
            outcome_line("claude", &run("2.1.90", "2.1.89")),
            "claude: version 2.1.90 is current now, before 2.1.89; running sessions keep theirs"
        );
        assert_eq!(
            outcome_line("claude", &run("2.1.89", "2.1.89")),
            "claude: version 2.1.89 is the current one; nothing changed"
        );
    }

    #[test]
    fn status_names_the_last_run() {
        let mut run = v1::AgentMaintenance {
            sandbox_id: "s1".to_owned(),
            task: "install".to_owned(),
            state: RUNNING.to_owned(),
            ..v1::AgentMaintenance::default()
        };
        assert_eq!(
            maintenance_line(&run),
            "  last install: running (sandbox s1)"
        );
        run.state = DONE.to_owned();
        run.version = "2.1.89".to_owned();
        assert_eq!(
            maintenance_line(&run),
            "  last install: done, version 2.1.89 (sandbox s1)"
        );
        run.state = "failed".to_owned();
        run.diagnostics.push(v1::Diagnostic {
            code: "AGENT_011".to_owned(),
            why: "the run ended with an error".to_owned(),
            ..v1::Diagnostic::default()
        });
        assert_eq!(
            maintenance_line(&run),
            "  last install: failed (sandbox s1)"
        );
        let shown = maintenance_json(&run);
        assert_eq!(shown["state"], "failed");
        assert_eq!(shown["sandbox_id"], "s1");
        assert_eq!(shown["diagnostics"][0]["code"], "AGENT_011");
    }

    #[test]
    fn an_answer_without_a_run_is_an_old_daemon() {
        let status = v1::AgentsStatus {
            agents: vec![v1::AgentDepot {
                agent: "claude".to_owned(),
                ..v1::AgentDepot::default()
            }],
            ..v1::AgentsStatus::default()
        };
        let failure = maintenance_of(&status, "claude").unwrap_err();
        assert_eq!(failure.diagnostic.code, codes::IPC_006);
        assert!(
            failure.diagnostic.why.contains("1.52"),
            "{}",
            failure.diagnostic.why
        );
    }
}
