//! `humanitl settings`: die Einstellungen als Bildschirm im Terminal
//! (GitHub #7, ADR-0024 Punkt 2).
//!
//! Der Bildschirm selbst steht in [`crate::tui::settings`]; hier steht, was
//! ihn mit dem Daemon und dem Rest des Befehls verbindet:
//!
//! - **Ohne Terminal oder mit `--json`** gibt es keinen Bildschirm, sondern
//!   den Hinweis auf `humanitl config get|set` und Exit 1.
//! - **Die Leitung** ([`spawn`]): Ein Auftrag der Laufzeit beantwortet die
//!   Anfragen des Bildschirms über gRPC, einer nach dem anderen, ein zweiter
//!   liest den Ereignisstrom und reicht `ConfigChanged` weiter. Die Schleife
//!   des Bildschirms wartet dabei auf keine Antwort.
//! - **Was den Bildschirm verlässt**: `config.toml` im `$EDITOR` öffnen (wie
//!   `humanitl config edit`) und den Daemon neu starten (wie die App: erst
//!   `retire-if-idle`, dann der Neustart des Dienstes). Danach steht der
//!   Bildschirm wieder da, an derselben Stelle, an einer neuen Verbindung.

use std::path::Path;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

use crate::cli::SettingsArgs;
use crate::cmd::{Context, EXIT_OK, Failure};
use crate::tui::settings::link::{self, Answer, Far, Link, Reply, Request};
use crate::tui::settings::{self, Exit, SettingsScreen, actions};
use crate::tui::signals::{self, Signal};
use crate::tui::{Options, Outcome, TerminalGuard, Theme, Unavailable};

/// Exit-Code nach `Ctrl+C` als Taste, wie eine Shell ihn für `SIGINT`
/// meldet.
const EXIT_INTERRUPTED: u8 = 130;

/// `humanitl settings [--project PATH]`.
///
/// # Errors
///
/// `CLI_004` ohne Terminal oder mit `--json`, `TERM_003`, wenn das Terminal
/// nicht taugt, `DAEMON_001`, wenn kein Daemon antwortet.
pub async fn run(ctx: &Context, args: &SettingsArgs) -> Result<u8, Failure> {
    if ctx.render.is_json() || !crate::tui::available() {
        return Err(Failure::new(no_terminal()));
    }
    let project = args.project.as_deref().map(|path| absolute(&ctx.cwd, path));
    let mut kept: Option<SettingsScreen> = None;
    loop {
        let client = match guarded(ctx.connect(), signals::take).await {
            Ok(client) => client,
            Err(Ok(code)) => return Ok(code),
            Err(Err(failure)) => return Err(failure),
        };
        let (link, worker) = spawn(client);
        let mut screen = match kept.take() {
            Some(mut screen) => {
                screen.relink(link);
                screen
            }
            None => SettingsScreen::new(link, project.clone()),
        };
        let outcome = tokio::task::block_in_place(|| show(&mut screen))?;
        drop(worker);
        match outcome {
            Outcome::Done => {}
            Outcome::Interrupted => return Ok(EXIT_INTERRUPTED),
            Outcome::Signal(signal) => {
                return Ok(signalled(Some(signal)).unwrap_or(EXIT_INTERRUPTED));
            }
        }
        // Außerhalb des Bildschirms setzt ein Signal nur seinen Merker
        // (`tui::signals`); nach Editor und Neustart wird er gelesen, und ein
        // Neustart, der hängt, lässt sich abbrechen.
        match screen.take_exit() {
            Exit::Quit => return Ok(EXIT_OK),
            Exit::Editor(file) => {
                let text = edit(ctx, &file);
                if let Some(code) = signalled(signals::take()) {
                    return Ok(code);
                }
                screen.note(actions::CONFIG_FILE, text);
            }
            Exit::Restart => match cancellable(restart(ctx), signals::take).await {
                Ok(text) => screen.note(actions::RESTART, text),
                Err(code) => return Ok(code),
            },
        }
        kept = Some(screen);
    }
}

/// Der Exit-Code nach einem Signal, das den Befehl beendet (`128 + n`);
/// `None` für Anhalten und Weiterlaufen.
fn signalled(signal: Option<Signal>) -> Option<u8> {
    match signal? {
        signal @ (Signal::Terminate | Signal::HangUp | Signal::Interrupt) => {
            Some(u8::try_from(128 + signal.number()).unwrap_or(EXIT_INTERRUPTED))
        }
        Signal::Stop | Signal::Continue => None,
    }
}

/// Wartet auf `work`, sieht dabei alle 50 ms nach Signalen und gibt bei
/// einem beendenden den Exit-Code zurück, ohne weiter zu warten.
async fn cancellable<T>(
    work: impl std::future::Future<Output = T>,
    mut poll: impl FnMut() -> Option<Signal>,
) -> Result<T, u8> {
    let mut work = std::pin::pin!(work);
    loop {
        tokio::select! {
            done = &mut work => return Ok(done),
            () = tokio::time::sleep(std::time::Duration::from_millis(50)) => {
                if let Some(code) = signalled(poll()) {
                    return Err(code);
                }
            }
        }
    }
}

/// Wie [`cancellable`] für eine Arbeit, die scheitern kann, etwa das
/// Verbinden nach Editor oder Neustart: Ein Signal, das kam, während sie
/// lief, gewinnt auch über ihren Fehler. `Err(Ok(code))` heißt beenden mit
/// `code`, `Err(Err(_))` der Fehler der Arbeit.
async fn guarded<T>(
    work: impl std::future::Future<Output = Result<T, Failure>>,
    mut poll: impl FnMut() -> Option<Signal>,
) -> Result<T, Result<u8, Failure>> {
    match cancellable(work, &mut poll).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(failure)) => Err(signalled(poll()).ok_or(failure)),
        Err(code) => Err(Ok(code)),
    }
}

/// Ein Ordner der Kommandozeile, absolut gegen das Arbeitsverzeichnis.
/// Kanonisiert wird im Daemon (`GetConfig.project_id`).
fn absolute(cwd: &Path, path: &Path) -> String {
    if path.is_absolute() {
        path.display().to_string()
    } else {
        cwd.join(path).display().to_string()
    }
}

/// Der Hinweis ohne Bildschirm.
fn no_terminal() -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(
            "humanitl settings is a full-screen view and needs a terminal on stdin and stdout \
             and no --json; humanitl config get shows a setting and humanitl config set KEY \
             VALUE changes it"
                .to_owned(),
        )
        .fix(FixAction::CopyCommand(settings::WITHOUT_TUI.to_owned()))
        .build()
}

/// Zeigt den Bildschirm, bis er endet.
fn show(screen: &mut SettingsScreen) -> Result<Outcome, Failure> {
    let options = Options {
        min: settings::MIN,
        mouse: false,
        without_tui: settings::WITHOUT_TUI,
    };
    let mut guard = TerminalGuard::enter(&options).map_err(|unavailable| match unavailable {
        Unavailable::NoTerminal => Failure::new(no_terminal()),
        Unavailable::Unsuitable(found) => Failure::new(*found),
        Unavailable::Failed(error) => Failure::new(terminal_failed(&error)),
    })?;
    crate::tui::run(&mut guard, screen, Theme::from_env(), settings::MIN)
        .map_err(|error| Failure::new(terminal_failed(&error)))
}

/// `TERM_003`: Das Terminal ließ sich nicht setzen oder nicht mehr
/// beschreiben.
fn terminal_failed(error: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(codes::TERM_003, Severity::Error)
        .why(format!(
            "the settings view could not use this terminal: {error}"
        ))
        .fix(FixAction::CopyCommand(settings::WITHOUT_TUI.to_owned()))
        .build()
}

/// `config.toml` im Editor, wie `humanitl config edit`; die Auskunft an der
/// Zeile sagt, ob die Datei danach taugt.
fn edit(ctx: &Context, file: &Path) -> String {
    let result = crate::cmd::config::editor_for(ctx)
        .and_then(|editor| crate::cmd::config::open_editor(ctx, &editor, file));
    if let Err(failure) = result {
        return failure.diagnostic.why;
    }
    match crate::cmd::config::check_file(file) {
        None => format!("{} is valid TOML.", file.display()),
        Some(diagnostic) => format!("{}: {}", diagnostic.code, diagnostic.why),
    }
}

/// Startet den Daemon neu wie die App: nur, wenn er sich verabschiedet,
/// weil keine Sitzung ihn hält.
async fn restart(ctx: &Context) -> String {
    use crate::cmd::daemon::retire::Outcome as Retire;

    match crate::cmd::daemon::retire::outcome(ctx).await {
        Retire::Busy | Retire::Unknown => {
            "An agent session holds the daemon, or it did not answer; it was not restarted."
                .to_owned()
        }
        Retire::Unsupported => "This daemon does not know the safe restart; humanitl daemon \
                                restart restarts it without asking."
            .to_owned(),
        Retire::Retired | Retire::NoDaemon => {
            match crate::cmd::daemon::run(ctx, &crate::cli::DaemonCmd::Restart).await {
                Ok(_) => "The background service was restarted.".to_owned(),
                Err(failure) => format!("{}: {}", failure.diagnostic.code, failure.diagnostic.why),
            }
        }
    }
}

/// Die Aufträge hinter der Leitung; beim Fallenlassen beendet.
pub struct Worker {
    /// Beantwortet die Anfragen.
    calls: tokio::task::JoinHandle<()>,
    /// Reicht `ConfigChanged` weiter.
    events: tokio::task::JoinHandle<()>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.calls.abort();
        self.events.abort();
    }
}

/// Eine Leitung an `client`.
#[must_use]
pub fn spawn(client: Client) -> (Link, Worker) {
    let (link, far) = link::pair();
    let Far { mut calls, replies } = far;
    let events_client = client.clone();
    let event_replies = replies.clone();
    let calls = tokio::spawn(async move {
        let mut client = client;
        while let Some(call) = calls.recv().await {
            let result = execute(&mut client, call.request).await;
            if replies
                .send(Reply {
                    row: call.row,
                    result,
                })
                .is_err()
            {
                return;
            }
        }
    });
    let events = tokio::spawn(watch(events_client, event_replies));
    (link, Worker { calls, events })
}

/// Eine Anfrage über gRPC.
async fn execute(client: &mut Client, request: Request) -> Result<Answer, Diagnostic> {
    let what = request.rpc();
    let fail = |status: tonic::Status| crate::cmd::status_diagnostic(&status, what);
    Ok(match request {
        Request::GetConfig(asked) => {
            Answer::Config(client.get_config(asked).await.map_err(fail)?.into_inner())
        }
        Request::SetConfig(asked) => {
            client.set_config(asked).await.map_err(fail)?;
            Answer::Written
        }
        Request::Projects(asked) => {
            let saves = matches!(asked.op, Some(v1::projects_request::Op::SaveChoice(_)));
            let answer = client.projects(asked).await.map_err(fail)?.into_inner();
            if saves {
                Answer::Saved(answer)
            } else {
                Answer::Project(answer)
            }
        }
        Request::LlmServers(asked) => {
            Answer::Servers(client.llm_servers(asked).await.map_err(fail)?.into_inner())
        }
        Request::LlmKey(asked) => {
            Answer::Key(client.llm_key(asked).await.map_err(fail)?.into_inner())
        }
        Request::ProbeLlm(asked) => {
            Answer::Probe(client.probe_llm(asked).await.map_err(fail)?.into_inner())
        }
        Request::SandboxBackends(asked) => Answer::Backends(
            client
                .sandbox_backends(asked)
                .await
                .map_err(fail)?
                .into_inner(),
        ),
        Request::Egress(asked) => {
            Answer::Egress(client.egress(asked).await.map_err(fail)?.into_inner())
        }
        Request::Agents(asked) => {
            Answer::Agents(client.agents(asked).await.map_err(fail)?.into_inner())
        }
        Request::GetInfo => Answer::Info(client.get_info(()).await.map_err(fail)?.into_inner()),
        Request::Update(asked) => {
            let mut stream = client.update(asked).await.map_err(fail)?.into_inner();
            loop {
                match stream.message().await.map_err(fail)? {
                    Some(v1::UpdateEvent {
                        event: Some(v1::update_event::Event::Status(status)),
                    }) => break Answer::Update(status),
                    Some(_) => {}
                    None => break Answer::Update(v1::UpdateStatus::default()),
                }
            }
        }
    })
}

/// Liest den Ereignisstrom und reicht jede Änderung der Konfiguration
/// weiter; nach `Lagged` gilt jede als möglich.
async fn watch(mut client: Client, replies: std::sync::mpsc::Sender<Reply>) {
    use v1::flow_event::{ConfigChanged, Event};

    let ended = Reply {
        row: None,
        result: Ok(Answer::LiveEnded),
    };
    let Ok(stream) = client.subscribe(v1::SubscribeRequest::default()).await else {
        let _ = replies.send(ended);
        return;
    };
    let mut stream = stream.into_inner();
    while let Ok(Some(event)) = stream.message().await {
        let changed = match event.event {
            Some(Event::ConfigChanged(changed)) => changed,
            Some(Event::Lagged(_)) => ConfigChanged::default(),
            _ => continue,
        };
        let reply = Reply {
            row: None,
            result: Ok(Answer::Changed(changed)),
        };
        if replies.send(reply).is_err() {
            return;
        }
    }
    let _ = replies.send(ended);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use super::{Failure, Signal, absolute, cancellable, guarded, signalled};

    /// Ein relativer Ordner wird gegen das Arbeitsverzeichnis absolut; ein
    /// absoluter bleibt.
    #[test]
    fn a_relative_project_becomes_absolute() {
        assert_eq!(absolute(Path::new("/w"), Path::new("a")), "/w/a");
        assert_eq!(absolute(Path::new("/w"), Path::new("/b")), "/b");
    }

    /// Ein beendendes Signal nach Editor oder Neustart wird `128 + n`;
    /// Anhalten und Weiterlaufen beenden nichts.
    #[test]
    fn a_terminating_signal_after_the_view_is_its_exit_code() {
        assert_eq!(signalled(Some(Signal::Terminate)), Some(143));
        assert_eq!(signalled(Some(Signal::HangUp)), Some(129));
        assert_eq!(signalled(Some(Signal::Interrupt)), Some(130));
        assert_eq!(signalled(Some(Signal::Continue)), None);
        assert_eq!(signalled(None), None);
    }

    /// Ein Neustart, der hängt, endet am Signal statt nie.
    #[tokio::test]
    async fn a_hanging_restart_ends_on_a_signal() {
        let mut asked = 0;
        // Mit Frist: Endete das Warten nicht am Signal, hinge sonst der Test.
        let ended = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            cancellable(std::future::pending::<()>(), || {
                asked += 1;
                (asked > 2).then_some(Signal::Interrupt)
            }),
        )
        .await;
        assert_eq!(ended, Ok(Err(130)), "the wait ends on the signal");
        let done = cancellable(async { 7 }, || None).await;
        assert_eq!(done, Ok(7));
    }

    /// Scheitert das Verbinden, während ein Signal kam, endet der Befehl am
    /// Signal und nicht mit dem Fehler; ohne Signal bleibt der Fehler.
    #[tokio::test]
    async fn a_signal_wins_over_a_failed_reconnect() {
        let failed = || async {
            Err::<(), Failure>(Failure::new(
                humanitl_core::Diagnostic::builder(
                    humanitl_core::diagnostics::codes::DAEMON_001,
                    humanitl_core::Severity::Blocking,
                )
                .why("no daemon".to_owned())
                .build(),
            ))
        };
        let wait = std::time::Duration::from_secs(5);
        let signal = tokio::time::timeout(wait, guarded(failed(), || Some(Signal::Terminate)))
            .await
            .expect("bounded");
        assert!(matches!(signal, Err(Ok(143))), "{signal:?}");
        let plain = tokio::time::timeout(wait, guarded(failed(), || None))
            .await
            .expect("bounded");
        assert!(matches!(plain, Err(Err(_))), "{plain:?}");
    }
}
