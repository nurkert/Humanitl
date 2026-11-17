//! Der Hinweis auf Cloud-Inferenz in der Kommandozeile (HUM-516).
//!
//! Ob der Agent seine Inferenz an einen Anbieter außerhalb dieses Rechners
//! schickt und ob der Mensch das für diese Kombination aus Agent und Anbieter
//! schon bestätigt hat, sagt der Daemon (`Status.cloud_inference`); die
//! Bestätigung setzt er auch (`SandboxRequest.acknowledge_cloud_inference`).
//! Hier wird nur gefragt und geschrieben (ADR-018):
//!
//! - **Vor dem ersten Start** mit dieser Kombination fragt `humanitl run`
//!   bzw. `go` auf dem Terminal einmal nach. Mit `--yes`, mit `--json` oder
//!   ohne Terminal steht der Hinweis als eine Zeile auf `stderr`, und
//!   niemand wartet; nur `--yes` und ein Ja auf die Frage gelten als
//!   Bestätigung.
//! - **Bei jedem Start** steht die kurze Zeile in Orange, ohne Farbe mit
//!   `NO_COLOR` oder ohne Terminal; ebenso in `humanitl sessions`.
//!
//! Der Hinweis hält nichts auf, was bestätigt ist.

use std::ffi::OsStr;
use std::io::{BufRead, IsTerminal as _, Write};

use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

use crate::cmd::Failure;

/// Die Minor, ab der der Daemon `Status.cloud_inference` und die Bestätigung
/// kennt.
pub(crate) const CLOUD_MINOR: u32 = 43;

/// Orange aus der 256-Farben-Palette (208): die Warnfarbe des Terminals,
/// nicht Rot, das Fehlern und Sperren gehört.
const ORANGE: &str = "\x1b[38;5;208m";

/// Zurück auf die Farbe des Terminals.
const RESET: &str = "\x1b[0m";

/// Wie der Hinweis vor dem ersten Start erscheint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Auf dem Terminal fragen und auf die Antwort warten.
    Ask,
    /// Eine Zeile auf `stderr` und bestätigen: `--yes`.
    Confirmed,
    /// Eine Zeile auf `stderr`, nicht warten, nicht bestätigen: ohne
    /// Terminal oder mit `--json`.
    Line,
}

/// Wie gefragt wird: `--yes` bestätigt, ohne Terminal auf beiden Seiten oder
/// mit `--json` steht nur die Zeile da.
#[must_use]
pub(crate) const fn mode(yes: bool, json: bool, terminal: bool) -> Mode {
    if yes {
        Mode::Confirmed
    } else if json || !terminal {
        Mode::Line
    } else {
        Mode::Ask
    }
}

/// Ob die Zeile Farbe bekommt: auf einem Terminal und ohne `NO_COLOR`
/// (no-color.org: gesetzt und nicht leer).
#[must_use]
pub(crate) fn colour_wanted(no_color: Option<&OsStr>, terminal: bool) -> bool {
    terminal && no_color.is_none_or(OsStr::is_empty)
}

/// Die kurze Zeile, in Orange, wenn `colour`.
#[must_use]
pub(crate) fn short_line(cloud: &v1::CloudInference, colour: bool) -> String {
    let text = format!(
        "[humanitl] {}",
        humanitl_config::inference::short_line(&crate::render::plain(&cloud.provider))
    );
    if colour {
        format!("{ORANGE}{text}{RESET}")
    } else {
        text
    }
}

/// Die kurze Zeile auf `stderr`, in Orange auf einem Terminal ohne
/// `NO_COLOR`.
pub(crate) fn print_short_line(cloud: &v1::CloudInference) {
    let colour = colour_wanted(
        std::env::var_os("NO_COLOR").as_deref(),
        std::io::stderr().is_terminal(),
    );
    eprintln!("{}", short_line(cloud, colour));
}

/// Der ganze Text des Hinweises, aus dem Befund des Daemons, sonst aus der
/// eigenen Fassung desselben Satzes.
fn full_text(cloud: &v1::CloudInference) -> String {
    let text = cloud
        .notice
        .as_ref()
        .map(|notice| notice.why.clone())
        .filter(|why| !why.is_empty())
        .unwrap_or_else(|| humanitl_config::inference::notice_text(&cloud.agent, &cloud.provider));
    crate::render::plain(&text)
}

/// Was der Mensch vor dem ersten Start gesagt hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Answer {
    /// Bestätigt: Der Daemon merkt es sich, und der Start geht weiter.
    Confirmed,
    /// Nur gesehen: Der Start geht weiter, und der nächste fragt wieder.
    Shown,
    /// Nicht bestätigt: Nichts startet.
    Declined,
}

/// Schreibt den Hinweis und liest, in [`Mode::Ask`], genau eine Zeile.
///
/// In [`Mode::Line`] und [`Mode::Confirmed`] liest diese Funktion nichts:
/// ohne Terminal wartet niemand.
pub(crate) fn announce(
    mode: Mode,
    cloud: &v1::CloudInference,
    out: &mut dyn Write,
    input: &mut dyn BufRead,
) -> Answer {
    let text = full_text(cloud);
    match mode {
        Mode::Line => {
            let _ = writeln!(out, "[humanitl] {text}");
            Answer::Shown
        }
        Mode::Confirmed => {
            let _ = writeln!(out, "[humanitl] {text} Confirmed with --yes.");
            Answer::Confirmed
        }
        Mode::Ask => {
            let _ = writeln!(out, "{text}");
            let _ = write!(
                out,
                "Start {} with inference at {}? It asks once per agent and provider. [y/N] ",
                crate::render::plain(&cloud.agent),
                crate::render::plain(&cloud.provider)
            );
            let _ = out.flush();
            let mut answer = String::new();
            let _ = input.read_line(&mut answer);
            if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
                Answer::Confirmed
            } else {
                Answer::Declined
            }
        }
    }
}

/// Fragt vor dem Start, wenn der Daemon für genau diesen Start eine
/// unbestätigte Cloud-Inferenz meldet (`SandboxRequest.check_cloud_inference`,
/// `cli_overrides` wie `--llm URL` eingeschlossen), und setzt in `start`, was
/// der Mensch gesagt hat: `acknowledge_cloud_inference` nach einem Ja oder
/// mit `--yes` (der Daemon merkt es beim Start), `cloud_notice_shown` nach
/// der Zeile ohne Terminal (er merkt nichts).
///
/// `Ok(false)` heißt: Der Mensch hat nicht bestätigt, und nichts darf
/// starten. Lässt sich die Frage nicht stellen (ein älterer Daemon, eine
/// Konfiguration, die nicht aufgeht), setzt diese Funktion nichts; der
/// Start lehnt dann selbst sichtbar ab (`LLM_011`) oder meldet seinen
/// Befund.
///
/// # Errors
///
/// Keine eigenen; die Signatur bleibt für die Aufrufer gleich.
pub(crate) async fn prepare_start(
    client: &mut Client,
    start: &mut v1::sandbox_request::Start,
    minor: u32,
    mode: Mode,
    local: (&humanitl_config::Paths, &humanitl_config::Config),
) -> Result<bool, Failure> {
    // Ein Daemon vor 1.43 (etwa nach einem Update noch nicht neu gestartet)
    // kennt weder das Tor noch die Frage. Dann fragt die Kommandozeile an
    // der Konfiguration dieses Aufrufs, `--llm` eingeschlossen, wie
    // `sandbox run`; merken kann ein solcher Daemon nichts (HUM-516, Review).
    if minor < CLOUD_MINOR {
        let (paths, config) = local;
        let Some(cloud) = old_daemon_notice(minor, paths, config) else {
            return Ok(true);
        };
        if decide(mode, &cloud) == Answer::Declined {
            eprintln!("[humanitl] not started; the notice was not confirmed");
            return Ok(false);
        }
        return Ok(true);
    }
    let Some(cloud) = checked(client, start.clone()).await else {
        return Ok(true);
    };
    if cloud.acknowledged {
        return Ok(true);
    }
    // Die Antwort nennt, wofür sie gilt; der Daemon nimmt sie nur für genau
    // diesen Agenten und Anbieter an.
    let shown = Some(v1::CloudNoticeFor {
        agent: cloud.agent.clone(),
        provider: cloud.provider.clone(),
    });
    match decide(mode, &cloud) {
        Answer::Shown => start.cloud_notice_shown = shown,
        Answer::Confirmed => start.acknowledge_cloud_inference = shown,
        Answer::Declined => {
            eprintln!("[humanitl] not started; the notice was not confirmed");
            return Ok(false);
        }
    }
    Ok(true)
}

/// Wie [`prepare_start`] für `humanitl sandbox run`, das die Sandbox im
/// eigenen Prozess startet: Die Lesart ist dieselbe Funktion wie im Daemon,
/// an der Konfiguration dieses Aufrufs, und die Bestätigung setzt der Daemon
/// über `SandboxRequest.acknowledge_cloud_inference` (ADR-018).
///
/// `Ok(false)` heißt: nicht bestätigt, nichts startet. Ohne `store` (ein
/// Daemon vor 1.43) wird eine Bestätigung nicht gemerkt, und der nächste
/// Start fragt wieder.
///
/// # Errors
///
/// Der Befund des Daemons, wenn er die Bestätigung nicht speichern konnte.
pub(crate) async fn confirm_local(
    client: &mut Client,
    paths: &humanitl_config::Paths,
    config: &humanitl_config::Config,
    mode: Mode,
    store: bool,
) -> Result<bool, Failure> {
    let Some(cloud) = local_notice(paths, config) else {
        return Ok(true);
    };
    match decide(mode, &cloud) {
        Answer::Shown => Ok(true),
        Answer::Declined => {
            eprintln!("[humanitl] not started; the notice was not confirmed");
            Ok(false)
        }
        Answer::Confirmed => {
            if store {
                acknowledge(client, &cloud).await?;
            }
            Ok(true)
        }
    }
}

/// Die Cloud-Inferenz, nach der die Kommandozeile selbst fragen muss, weil
/// der Daemon vor 1.43 spricht und weder Tor noch Frage kennt (HUM-516,
/// Review); `None` ab 1.43, dann entscheidet der Daemon.
pub(crate) fn old_daemon_notice(
    minor: u32,
    paths: &humanitl_config::Paths,
    config: &humanitl_config::Config,
) -> Option<v1::CloudInference> {
    (minor < CLOUD_MINOR)
        .then(|| local_notice(paths, config))
        .flatten()
}

/// Die unbestätigte Cloud-Inferenz dieser Konfiguration für `sandbox run`,
/// oder `None`, wenn sie lokal bleibt oder bestätigt ist. Dieselbe Lesart wie
/// das Tor des Daemons (`humanitl_ipc::cloud::gate`).
pub(crate) fn local_notice(
    paths: &humanitl_config::Paths,
    config: &humanitl_config::Config,
) -> Option<v1::CloudInference> {
    let found = humanitl_sandbox::agent::cloud::cloud_inference(config)?;
    let cloud = humanitl_ipc::cloud::to_proto(paths, &found);
    (!cloud.acknowledged).then_some(cloud)
}

/// Der Hinweis auf `stderr`, in [`Mode::Ask`] mit der Frage auf `stdin`.
fn decide(mode: Mode, cloud: &v1::CloudInference) -> Answer {
    let mut err = std::io::stderr();
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    announce(mode, cloud, &mut err, &mut input)
}

/// `Status.cloud_inference` eines Starts mit genau diesen Angaben, oder
/// `None`, wenn die Inferenz lokal bliebe oder die Frage nicht gelang.
async fn checked(
    client: &mut Client,
    start: v1::sandbox_request::Start,
) -> Option<v1::CloudInference> {
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::CheckCloudInference(start)),
        })
        .await
        .ok()?
        .into_inner();
    let mut found = None;
    while let Ok(Some(event)) = stream.message().await {
        if let Some(v1::sandbox_event::Event::CloudInferenceCheck(check)) = event.event {
            found = check.cloud_inference;
        }
    }
    found
}

/// Lässt den Daemon die Bestätigung merken.
async fn acknowledge(client: &mut Client, cloud: &v1::CloudInference) -> Result<(), Failure> {
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::AcknowledgeCloudInference(
                v1::sandbox_request::AcknowledgeCloudInference {
                    agent: cloud.agent.clone(),
                    provider: cloud.provider.clone(),
                },
            )),
        })
        .await
        .map_err(|status| {
            Failure::new(crate::cmd::status_diagnostic(
                &status,
                "Sandbox(AcknowledgeCloudInference)",
            ))
        })?
        .into_inner();
    while let Ok(Some(event)) = stream.message().await {
        if let Some(v1::sandbox_event::Event::Diagnostic(wire)) = event.event
            && let Some(diagnostic) = crate::cmd::from_proto(&wire)
        {
            return Err(Failure::new(diagnostic));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read;

    use super::*;

    fn cloud() -> v1::CloudInference {
        v1::CloudInference {
            agent: "opencode".to_owned(),
            provider: "api.anthropic.com".to_owned(),
            acknowledged: false,
            notice: None,
        }
    }

    /// Ein Eingang, der jeden Leseversuch als Fehler des Tests meldet: Ohne
    /// Terminal wartet niemand auf eine Antwort.
    struct NoInput;

    impl Read for NoInput {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("the notice waited for an answer without a terminal");
        }
    }

    impl BufRead for NoInput {
        fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
            panic!("the notice waited for an answer without a terminal");
        }
        fn consume(&mut self, _: usize) {}
    }

    #[test]
    fn without_a_terminal_one_line_and_no_wait() {
        assert_eq!(mode(false, false, false), Mode::Line);
        assert_eq!(mode(false, true, true), Mode::Line, "--json never asks");
        let mut out = Vec::new();
        let answer = announce(Mode::Line, &cloud(), &mut out, &mut NoInput);
        assert_eq!(answer, Answer::Shown, "shown is no confirmation");
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.lines().count(), 1, "{text}");
        assert!(text.contains("api.anthropic.com"), "{text}");
        assert!(text.contains("deliberately allowed"), "{text}");
    }

    #[test]
    fn yes_confirms_without_waiting() {
        assert_eq!(mode(true, false, false), Mode::Confirmed);
        assert_eq!(mode(true, false, true), Mode::Confirmed);
        let mut out = Vec::new();
        let answer = announce(Mode::Confirmed, &cloud(), &mut out, &mut NoInput);
        assert_eq!(answer, Answer::Confirmed);
        assert_eq!(String::from_utf8(out).unwrap().lines().count(), 1);
    }

    #[test]
    fn at_a_terminal_only_yes_confirms() {
        assert_eq!(mode(false, false, true), Mode::Ask);
        for (typed, expected) in [
            ("y\n", Answer::Confirmed),
            ("YES\n", Answer::Confirmed),
            ("\n", Answer::Declined),
            ("n\n", Answer::Declined),
            ("", Answer::Declined),
        ] {
            let mut out = Vec::new();
            let mut input = std::io::Cursor::new(typed.as_bytes());
            assert_eq!(
                announce(Mode::Ask, &cloud(), &mut out, &mut input),
                expected,
                "{typed:?}"
            );
            let text = String::from_utf8(out).unwrap();
            assert!(text.contains("[y/N]"), "{text}");
        }
    }

    #[test]
    fn orange_only_on_a_terminal_without_no_color() {
        assert!(colour_wanted(None, true));
        assert!(colour_wanted(Some(OsStr::new("")), true));
        assert!(!colour_wanted(Some(OsStr::new("1")), true));
        assert!(!colour_wanted(None, false));
        let painted = short_line(&cloud(), true);
        assert!(painted.starts_with(ORANGE) && painted.ends_with(RESET));
        assert!(!painted.contains("\x1b[31m"), "never red");
        let plain = short_line(&cloud(), false);
        assert_eq!(
            plain,
            "[humanitl] Cloud inference: data goes to api.anthropic.com"
        );
    }

    /// `humanitl sandbox run -- opencode` startet am Tor des Daemons vorbei
    /// und fragt deshalb selbst (HUM-516, Review): ein unbestätigter
    /// Cloud-Endpunkt verlangt den Hinweis, ein bestätigter und ein lokaler
    /// nicht.
    #[test]
    fn sandbox_run_needs_the_notice_for_an_unconfirmed_cloud_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.path().display().to_string()),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").display().to_string(),
            ),
        ]));
        let mut config = humanitl_config::Config::default();
        config.llm.endpoint = "https://api.openai.com/v1".parse().ok();
        let notice = local_notice(&paths, &config).expect("cloud and unconfirmed");
        assert_eq!(notice.provider, "api.openai.com");

        humanitl_config::inference::acknowledge(&paths, "opencode", "api.openai.com").unwrap();
        assert!(local_notice(&paths, &config).is_none(), "confirmed once");

        config.llm.endpoint = "http://127.0.0.1:11434".parse().ok();
        assert!(local_notice(&paths, &config).is_none(), "local");
    }

    /// Ein Daemon vor 1.43 kennt das Tor nicht (HUM-516, Review): Dann fragt
    /// `run`/`go` an der eigenen Konfiguration, `--llm` eingeschlossen; ab
    /// 1.43 fragt der Daemon.
    #[test]
    fn an_old_daemon_leaves_the_question_to_the_command_line() {
        let dir = tempfile::tempdir().unwrap();
        let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.path().display().to_string()),
            (
                "XDG_DATA_HOME",
                dir.path().join("data").display().to_string(),
            ),
        ]));
        let mut config = humanitl_config::Config::default();
        config.llm.endpoint = "https://api.openai.com/v1".parse().ok();
        let asked = old_daemon_notice(42, &paths, &config).expect("1.42 has no gate");
        assert_eq!(asked.provider, "api.openai.com");
        assert!(
            old_daemon_notice(CLOUD_MINOR, &paths, &config).is_none(),
            "from 1.43 on the daemon decides"
        );
        config.llm.endpoint = "http://192.168.1.20:11434".parse().ok();
        assert!(old_daemon_notice(42, &paths, &config).is_none(), "local");
    }

    /// Ein Anbieter, den ein fremder Daemon schickte, bringt keine
    /// Steuerfolge in das Terminal.
    #[test]
    fn a_provider_with_escapes_is_shown_flat() {
        let mut hostile = cloud();
        hostile.provider = "evil\x1b]0;x\x07.com".to_owned();
        assert!(!short_line(&hostile, false).contains('\x1b'));
    }
}
