//! Der erste `humanitl go` in einem Ordner, den das Register nicht kennt
//! (GitHub #9, ADR-0024 Punkt 3).
//!
//! Ob der Ordner neu ist, sagt der Daemon: `Projects.proposal` ohne
//! `project_id` (HUM-536). Scheitert die Frage oder kommt kein Vorschlag
//! zurück, startet nichts: Ein unbekannter Ordner startet nie ohne gezeigte
//! oder gespeicherte Wahl. Nur zwei Wege sind davon ausgenommen und starten
//! wie vor dem Register: ein Daemon vor Vertrag 1.59, der nicht am Ordner
//! speichert (die Zeile von #8), und einer, der `Projects` gar nicht kennt
//! (`Unimplemented`).
//!
//! Für einen neuen Ordner gibt es genau einen von zwei Wegen, beide vor der
//! Ladeanimation und vor dem Start:
//!
//! - **Der Bildschirm** ([`crate::tui::first_start`]) an einem Terminal auf
//!   `stdin` und `stdout`, ohne `--json` und ohne `--yes`. `Enter` speichert
//!   die Wahl des gezeigten Vorschlags (`Projects.save_choice` am Ordner,
//!   Vertrag 1.59) und startet; `Esc`, `q` und `Ctrl+C` brechen ab, nichts
//!   ist gespeichert, nichts startet, Exit 130. Lehnt der Daemon das
//!   Speichern ab, steht der Bildschirm mit dem Befund wieder da.
//! - **Eine Zeile** auf `stderr` sonst, auch wenn der Bildschirm nicht geht
//!   (`TERM_003`): Die Vorgaben werden gespeichert, und erst danach sagt die
//!   Zeile, welche es sind und wie man sie ändert. Dann startet `go`; den
//!   Hinweis, wo Anfragen ohne Regel entschieden werden, lässt dieser Lauf
//!   weg, die Zeile nennt den Frage-Modus schon.
//!
//! Der nächste `go` im selben Ordner findet die Wahl im Register und zeigt
//! weder Bildschirm noch Zeile.
//!
//! # Zwischen Bildschirm und Ladeanimation
//!
//! Der Bildschirm endet, bevor irgendetwas anderes das Terminal anfasst; sein
//! Wächter gibt es zurück. Dann wird gespeichert, und **erst danach**,
//! unmittelbar vor der Übergabe an Frage zur Cloud-Inferenz (HUM-516) und
//! Start, wird die Eingabe verworfen, die noch ansteht: was crossterm schon
//! gelesen hat und was im Terminal wartet (`tcflush`). Ein zweites `Enter`,
//! das während des Speicherns kam, beantwortet so weder diese Frage noch
//! erreicht es den Agenten.
//!
//! Solange ein Bildschirm stand, fangen seine Merker `SIGTERM`, `SIGHUP` und
//! `SIGINT` (`tui::signals`); die Signale von `tokio` stehen erst im Lauf.
//! Ein Signal, das während des Speicherns oder danach kam, beendet deshalb
//! den Befehl hier mit `128 + n` ([`signalled`]), bevor etwas startet.

use std::path::Path;
use std::time::Duration;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

use crate::cmd::{Context, EXIT_USER, Failure, status_diagnostic};
use crate::tui::first_start::view::defaults_text;
use crate::tui::first_start::{self, Exit, FirstStartScreen};
use crate::tui::signals::{self, Signal};
use crate::tui::{Options, Outcome, TerminalGuard, Theme, Unavailable};

/// Ab dieser Minor speichert `Projects.save_choice` am Ordner (HUM-538).
pub(crate) const FOLDER_CHOICE_MINOR: u32 = 59;

/// Exit-Code eines Abbruchs, wie eine Shell ihn für `SIGINT` meldet.
const EXIT_CANCELLED: u8 = 130;

/// So viele Ereignisse liest das Verwerfen höchstens.
const DISCARD_LIMIT: usize = 4096;

/// Wie es nach dem Erststart weitergeht.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Next {
    /// Starten. `said`: Die Zeile des Erststarts steht schon da, und der
    /// Hinweis zu Anfragen ohne Regel entfällt.
    Start {
        /// Ob die Zeile des Erststarts geschrieben wurde.
        said: bool,
    },
    /// Nichts starten, Exit mit diesem Code.
    Stop(u8),
}

/// Wie der Bildschirm endete.
enum Shown {
    /// Die Wahl ist gespeichert; `go` startet.
    Saved,
    /// Abgebrochen oder durch ein Signal beendet: Exit mit diesem Code.
    Ended(u8),
    /// Kein Bildschirm (zu klein, keine Cursorsteuerung, kein Rohmodus);
    /// der Befund, wenn es einen gibt, und dann der Weg ohne Bildschirm.
    Unavailable(Option<Box<Diagnostic>>),
}

/// Der Erststart, wenn das Register den Ordner nicht kennt.
///
/// # Errors
///
/// Der Befund des Daemons, wenn er keinen Vorschlag macht oder die Wahl ohne
/// Bildschirm nicht speichert (Exit 1), und `TERM_003`, wenn das Terminal
/// mitten im Bildschirm versagt.
pub(crate) async fn first_start(
    ctx: &Context,
    client: &mut Client,
    (work_dir, overrides): (&Path, &[v1::sandbox_request::CliOverride]),
    minor: u32,
    yes: bool,
) -> Result<Next, Failure> {
    if minor < FOLDER_CHOICE_MINOR {
        // Ein älterer Daemon speichert nicht am Ordner; er bekommt die Zeile
        // von #8 und keine Wahl.
        super::announce_new_project(ctx, client, work_dir, minor).await;
        return Ok(Next::Start { said: false });
    }
    let path = work_dir.display().to_string();
    // Mit den Werten der Sitzung, wie der Start sie schickt (HUM-567): Die
    // Vorgaben zeigen dann den Start, der folgt. Gespeichert werden sie nie
    // (`StartProposal.choice` ist die Wahl ohne sie).
    let cli_overrides = overrides.to_vec();
    let Some(proposal) = ask_proposal(client, &path, &cli_overrides).await? else {
        return Ok(Next::Start { said: false });
    };
    if !proposal.project_id.is_empty() {
        return Ok(Next::Start { said: false });
    }
    if screen_wanted(ctx.render.is_json(), yes, crate::tui::available()) {
        match Box::pin(show(client, &path, proposal.clone(), cli_overrides)).await? {
            Shown::Saved => {
                if let Some(code) = signalled(signals::take()) {
                    return Ok(Next::Stop(code));
                }
                // Erst jetzt, nach dem Speichern: Was während des Speicherns
                // getippt wurde, erreicht weder die Frage zur Cloud-Inferenz
                // noch den Agenten.
                discard_events();
                flush_input();
                return Ok(Next::Start { said: false });
            }
            Shown::Ended(code) => {
                eprintln!("[humanitl] not started; nothing was saved");
                return Ok(Next::Stop(code));
            }
            Shown::Unavailable(found) => {
                if let Some(found) = found {
                    ctx.render
                        .note(crate::render::diagnostic_block(&found).trim_end());
                }
            }
        }
    }
    save(client, &path, proposal.choice.clone()).await?;
    // Auch mit `--json` und `-q`: Die Zeile sagt, welche Wahl dieser Aufruf
    // für das Projekt gespeichert hat, wie der Hinweis zur Cloud-Inferenz.
    eprintln!("{}", line(&proposal, work_dir));
    if let Some(code) = signalled(signals::take()) {
        return Ok(Next::Stop(code));
    }
    Ok(Next::Start { said: true })
}

/// `Projects.proposal` für den Ordner; `None` nur für einen Daemon, der
/// `Projects` nicht kennt (`Unimplemented`): Er startet wie vor dem Register.
///
/// # Errors
///
/// Der Befund des Daemons, wenn er die Frage ablehnt, und `DAEMON_002`, wenn
/// seine Antwort keinen Vorschlag trägt; beide mit Exit 1, und nichts
/// startet.
async fn ask_proposal(
    client: &mut Client,
    path: &str,
    cli_overrides: &[v1::sandbox_request::CliOverride],
) -> Result<Option<v1::StartProposal>, Failure> {
    let asked = client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Proposal(
                v1::projects_request::Proposal {
                    path: path.to_owned(),
                    project_id: String::new(),
                    values: Vec::new(),
                    cli_overrides: cli_overrides.to_vec(),
                },
            )),
        })
        .await;
    let answer = match asked {
        Ok(answer) => answer.into_inner(),
        Err(status) if status.code() == tonic::Code::Unimplemented => return Ok(None),
        Err(status) => {
            return Err(Failure::with_exit(
                status_diagnostic(&status, "Projects(Proposal)"),
                EXIT_USER,
            ));
        }
    };
    answer.proposal.map(Some).ok_or_else(|| {
        Failure::with_exit(
            Diagnostic::builder(codes::DAEMON_002, Severity::Error)
                .why(
                    "the daemon answered Projects(Proposal) without a start proposal, so the \
                     first start in this folder cannot be shown or saved; nothing was started"
                        .to_owned(),
                )
                .fix(FixAction::CopyCommand("humanitl daemon restart".to_owned()))
                .build(),
            EXIT_USER,
        )
    })
}

/// Ob der Bildschirm erscheint: ohne `--json`, ohne `--yes` und an einem
/// Terminal auf `stdin` und `stdout`.
const fn screen_wanted(json: bool, yes: bool, terminal: bool) -> bool {
    !json && !yes && terminal
}

/// Der Exit-Code eines Signals, das den Befehl beendet (`128 + n`); `None`
/// für keines, für Anhalten und für Weiterlaufen.
#[must_use]
pub(crate) fn signalled(signal: Option<Signal>) -> Option<u8> {
    match signal? {
        signal @ (Signal::Terminate | Signal::HangUp | Signal::Interrupt) => {
            Some(u8::try_from(128 + signal.number()).unwrap_or(EXIT_CANCELLED))
        }
        Signal::Stop | Signal::Continue => None,
    }
}

/// Die eine Zeile des Wegs ohne Bildschirm, nachdem gespeichert ist.
fn line(proposal: &v1::StartProposal, work_dir: &Path) -> String {
    let name = crate::render::plain(&proposal.display_name);
    format!(
        "[humanitl] first start in {name}, defaults saved: {}. Change them with humanitl \
         settings --project {}",
        crate::render::plain(&defaults_text(proposal)),
        shell_word(work_dir.as_os_str().as_encoded_bytes())
    )
}

/// Die Bytes eines Pfads als genau ein Wort der Shell auf einer Zeile, in
/// der Quotierung von bash und zsh (`$'…'` kennt eine POSIX-`sh` wie `dash`
/// nicht).
///
/// Unverändert, wenn nur Zeichen darin stehen, die keine Shell deutet; in
/// einfachen Anführungszeichen, wenn alles druckbar ist; sonst (ein
/// Steuerzeichen wie ein Zeilenumbruch, ein unsichtbares Zeichen, kein
/// UTF-8) als `$'…'` mit `\xHH` für jedes Byte außerhalb von druckbarem
/// ASCII. Gesäubert wird hier nichts: Das Wort ist der Pfad, den der Befehl
/// braucht, nicht eine Anzeige davon.
fn shell_word(bytes: &[u8]) -> String {
    let bare = !bytes.is_empty()
        && bytes.iter().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'/' | b'.' | b'_' | b'-' | b'+' | b',' | b':' | b'@')
        });
    if bare {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    // `clean` nimmt Steuerzeichen und unsichtbare Zeichen heraus; gleicht
    // der Text danach sich selbst, ist jedes Zeichen druckbar.
    if let Ok(text) = std::str::from_utf8(bytes)
        && crate::tui::settings::model::clean(text) == text
    {
        return format!("'{}'", text.replace('\'', "'\\''"));
    }
    let mut word = String::from("$'");
    for &byte in bytes {
        match byte {
            b'\'' => word.push_str("\\'"),
            b'\\' => word.push_str("\\\\"),
            0x20..=0x7e => word.push(char::from(byte)),
            other => {
                let _ = std::fmt::Write::write_fmt(&mut word, format_args!("\\x{other:02x}"));
            }
        }
    }
    word.push('\'');
    word
}

/// Speichert `choice` als Projektwahl des Ordners `path`.
///
/// # Errors
///
/// Der Befund des Daemons, etwa `CONFIG_024` für einen Wert, den eine
/// Projektwahl nicht tragen darf.
async fn save(
    client: &mut Client,
    path: &str,
    choice: Vec<v1::ChoiceValue>,
) -> Result<(), Failure> {
    client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::SaveChoice(
                v1::projects_request::SaveChoice {
                    project_id: String::new(),
                    values: choice,
                    path: path.to_owned(),
                },
            )),
        })
        .await
        .map(|_| ())
        .map_err(|status| Failure::new(status_diagnostic(&status, "Projects(SaveChoice)")))
}

/// Zeigt den Bildschirm, bis gespeichert oder abgebrochen ist.
///
/// # Errors
///
/// `TERM_003`, wenn das Terminal mitten im Bildschirm versagt oder nach
/// einer Ablehnung des Speicherns keinen Bildschirm mehr hergibt.
async fn show(
    client: &Client,
    path: &str,
    proposal: v1::StartProposal,
    cli_overrides: Vec<v1::sandbox_request::CliOverride>,
) -> Result<Shown, Failure> {
    let (link, worker) = crate::cmd::settings::spawn(client.clone());
    let mut screen = FirstStartScreen::new(link, path.to_owned(), proposal, cli_overrides);
    let mut saver = client.clone();
    let mut first = true;
    let shown = loop {
        let outcome = match tokio::task::block_in_place(|| present(&mut screen)) {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(error)) => return Err(Failure::new(terminal_failed(&error))),
            Err(unavailable) if first => break unavailable_reason(unavailable),
            Err(unavailable) => {
                let found = match unavailable {
                    Unavailable::Unsuitable(found) => *found,
                    Unavailable::NoTerminal => terminal_failed(&std::io::Error::other(
                        "stdin or stdout is no longer a terminal",
                    )),
                    Unavailable::Failed(error) => terminal_failed(&error),
                };
                return Err(Failure::new(found));
            }
        };
        first = false;
        match outcome {
            Outcome::Done => {}
            Outcome::Interrupted => break Shown::Ended(EXIT_CANCELLED),
            Outcome::Signal(signal) => {
                break Shown::Ended(signalled(Some(signal)).unwrap_or(EXIT_CANCELLED));
            }
        }
        match screen.take_exit() {
            Exit::Cancel => break Shown::Ended(EXIT_CANCELLED),
            Exit::Start(choice) => match save(&mut saver, path, choice).await {
                Ok(()) => break Shown::Saved,
                Err(failure) => screen.refused(failure.diagnostic),
            },
        }
    };
    drop(worker);
    Ok(shown)
}

/// Warum es keinen Bildschirm gibt, für den Weg ohne ihn.
fn unavailable_reason(unavailable: Unavailable) -> Shown {
    match unavailable {
        Unavailable::NoTerminal => Shown::Unavailable(None),
        Unavailable::Unsuitable(found) => Shown::Unavailable(Some(found)),
        Unavailable::Failed(error) => Shown::Unavailable(Some(Box::new(terminal_failed(&error)))),
    }
}

/// Ein Durchgang des Bildschirms: Wächter auf, Schleife, was crossterm schon
/// gelesen hat verwerfen, Wächter zu. Die Eingabe im Terminal verwirft erst
/// [`first_start()`] nach dem Speichern.
fn present(screen: &mut FirstStartScreen) -> Result<std::io::Result<Outcome>, Unavailable> {
    let options = Options {
        min: first_start::MIN,
        mouse: false,
        without_tui: first_start::WITHOUT_TUI,
    };
    let mut guard = TerminalGuard::enter(&options)?;
    let outcome = crate::tui::run(&mut guard, screen, Theme::from_env(), first_start::MIN);
    discard_events();
    drop(guard);
    Ok(outcome)
}

/// Verwirft, was crossterm schon gelesen und noch nicht ausgeliefert hat.
fn discard_events() {
    for _ in 0..DISCARD_LIMIT {
        match crossterm::event::poll(Duration::ZERO) {
            Ok(true) => {
                if crossterm::event::read().is_err() {
                    return;
                }
            }
            _ => return,
        }
    }
}

/// Verwirft, was im Terminal an Eingabe ansteht und noch niemand gelesen
/// hat.
fn flush_input() {
    let _ = rustix::termios::tcflush(
        rustix::stdio::stdin(),
        rustix::termios::QueueSelector::IFlush,
    );
}

/// `TERM_003`: Das Terminal ließ sich nicht setzen oder nicht mehr
/// beschreiben.
fn terminal_failed(error: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(codes::TERM_003, Severity::Error)
        .why(format!(
            "the first-start screen could not use this terminal: {error}"
        ))
        .fix(FixAction::CopyCommand(first_start::WITHOUT_TUI.to_owned()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Der Bildschirm nur an einem Terminal, ohne `--json` und ohne `--yes`.
    #[test]
    fn the_screen_needs_a_terminal_and_neither_json_nor_yes() {
        assert!(screen_wanted(false, false, true));
        assert!(!screen_wanted(true, false, true), "--json");
        assert!(!screen_wanted(false, true, true), "--yes");
        assert!(!screen_wanted(false, false, false), "no terminal");
    }

    /// Die Zeile nennt Ordnername, Vorgaben ohne Leitwort, dass sie
    /// gespeichert sind, und einen Befehl, der auch mit Leerzeichen im Pfad
    /// abzutippen ist.
    #[test]
    fn the_line_names_the_saved_defaults_and_how_to_change_them() {
        let proposal = v1::StartProposal {
            display_name: "my shop".to_owned(),
            defaults_line: "Defaults: agent OpenCode, folder disposable".to_owned(),
            ..v1::StartProposal::default()
        };
        assert_eq!(
            line(&proposal, Path::new("/home/u/my shop")),
            "[humanitl] first start in my shop, defaults saved: agent OpenCode, folder \
             disposable. Change them with humanitl settings --project '/home/u/my shop'"
        );
    }

    /// Das Wort ist der Pfad aus seinen Bytes, auf einer Zeile: ein
    /// Zeilenumbruch, ein unsichtbares Zeichen und kein UTF-8 werden
    /// `$'…'` mit `\xHH`, nie ein anderer Pfad.
    ///
    /// Mutation proof: quoting the sanitized display path instead (the old
    /// way) turns `"/tmp/a\nb"` into `'/tmp/a b'`, another folder.
    #[test]
    fn the_shell_word_keeps_every_byte_on_one_line() {
        assert_eq!(shell_word(b"/home/u/shop-1"), "/home/u/shop-1");
        assert_eq!(shell_word(b"/home/u/it's"), "'/home/u/it'\\''s'");
        assert_eq!(shell_word("/home/u/Bücher".as_bytes()), "'/home/u/Bücher'");
        assert_eq!(shell_word(b"/tmp/a\nb"), "$'/tmp/a\\x0ab'");
        assert_eq!(
            shell_word("/tmp/x\u{202e}y".as_bytes()),
            "$'/tmp/x\\xe2\\x80\\xaey'"
        );
        assert_eq!(shell_word(b"/tmp/\xff'\\"), "$'/tmp/\\xff\\'\\\\'");
    }

    /// Ein beendendes Signal ist `128 + n`; Anhalten und Weiterlaufen
    /// beenden nichts.
    #[test]
    fn a_terminating_signal_is_its_exit_code() {
        assert_eq!(signalled(Some(Signal::Terminate)), Some(143));
        assert_eq!(signalled(Some(Signal::Interrupt)), Some(130));
        assert_eq!(signalled(Some(Signal::HangUp)), Some(129));
        assert_eq!(signalled(Some(Signal::Stop)), None);
        assert_eq!(signalled(None), None);
    }
}
