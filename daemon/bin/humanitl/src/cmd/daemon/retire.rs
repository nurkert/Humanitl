//! `humanitl daemon retire-if-idle` (HUM-284).
//!
//! Bittet den laufenden Daemon, sich zu verabschieden, wenn keine Sitzung ihn
//! hält. Der Daemon entscheidet das unter dem Schloss, unter dem ein Start
//! seinen Anspruch nimmt (`SandboxRequest.retire_if_idle`); zwischen „keine
//! Sitzung" und dem Ende kann deshalb kein Start mehr anfangen. Danach endet
//! er mit dem gewöhnlichen Abschied, und wer ihn gebeten hat, startet ihn neu
//! (`systemctl --user restart humanitld.service`), oder die nächste
//! Verbindung startet ihn über den Socket.
//!
//! Aufrufer sind `postinst` nach einem Paket-Update
//! (`packaging/deb/maintscript-users.sh`) und der Knopf „Restart daemon" der
//! Statusleiste. Beide lesen das Feld `outcome`:
//!
//! - `retired`: Der Riegel steht, der Daemon endet jetzt.
//! - `busy`: Eine Sitzung hält ihn; er läuft weiter.
//! - `unsupported`: Ein Daemon vor Vertrag 1.19 kennt die Bitte nicht. Für
//!   ihn bleibt nur die alte Frage über `daemon status` mit dem Fenster
//!   zwischen Frage und Neustart; das betrifft genau das Update von 0.0.6.
//! - `none`: Es läuft kein Daemon: Token oder Socket fehlen. Nichts zu tun.
//! - `unknown`: Der Daemon hat nicht lesbar geantwortet, oder die Verbindung
//!   scheiterte, obwohl Token und Socket daliegen. Zählt wie `busy`.
//!
//! Die Kommandozeile weckt keinen Daemon hinter dem Socket, um ihn gleich
//! wieder zu beenden (`Context::connect_running`).

use std::time::Duration;

use humanitl_ipc::client::Client;
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure};

/// Wie lange die Antwort dauern darf.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Die erste Minor des Vertrags, die `retire_if_idle` kennt.
pub const RETIRE_MINOR: u32 = 19;

/// Was aus der Bitte wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Der Daemon endet jetzt.
    Retired,
    /// Eine Sitzung hält ihn.
    Busy,
    /// Der Daemon kennt die Bitte nicht.
    Unsupported,
    /// Es läuft kein Daemon.
    NoDaemon,
    /// Keine lesbare Antwort.
    Unknown,
}

impl Outcome {
    /// Der Wert im Feld `outcome`. Die Namen sind Vertrag mit `postinst` und
    /// der App.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Retired => "retired",
            Self::Busy => "busy",
            Self::Unsupported => "unsupported",
            Self::NoDaemon => "none",
            Self::Unknown => "unknown",
        }
    }

    /// Die Zeile für Menschen.
    const fn sentence(self) -> &'static str {
        match self {
            Self::Retired => "no session holds the daemon; it ends now",
            Self::Busy => "an agent session holds the daemon; it keeps running",
            Self::Unsupported => {
                "this daemon is older than contract 1.19 and does not know the request"
            }
            Self::NoDaemon => "no daemon is running",
            Self::Unknown => "the daemon did not answer readably; treat it as busy",
        }
    }

    /// Was die erste Antwort auf `retire_if_idle` sagt.
    ///
    /// Ein `Retire` ist die Antwort; eine Momentaufnahme kommt von einem
    /// Daemon, der die Operation nicht kennt und sie wie `status` liest.
    #[must_use]
    pub fn of(event: Option<&v1::sandbox_event::Event>) -> Self {
        match event {
            Some(v1::sandbox_event::Event::Retire(retire)) if retire.retired => Self::Retired,
            Some(v1::sandbox_event::Event::Retire(_)) => Self::Busy,
            Some(v1::sandbox_event::Event::Status(_)) => Self::Unsupported,
            _ => Self::Unknown,
        }
    }
}

/// `daemon retire-if-idle`.
///
/// # Errors
///
/// Keine eigenen: Jede Antwort, auch keine, ist ein [`Outcome`].
pub async fn retire_if_idle(ctx: &Context) -> Result<u8, Failure> {
    let outcome = if daemon_absent(
        ctx.paths.token_path().exists(),
        ctx.paths.daemon_socket().exists(),
    ) {
        Outcome::NoDaemon
    } else {
        match ctx.connect_running().await {
            Ok(mut client) => ask(&mut client).await,
            Err(_) => Outcome::Unknown,
        }
    };
    if ctx.render.is_json() {
        ctx.render.value(&json!({ "outcome": outcome.name() }));
    } else {
        println!("{}: {}", outcome.name(), outcome.sentence());
    }
    Ok(EXIT_OK)
}

/// Ob kein Daemon läuft: Ihm fehlt das Token oder der Socket.
///
/// Ein laufender Daemon hat beides; einer hinter `humanitld.socket`, der
/// gerade nicht läuft, hat den Socket von systemd, aber kein Token. Jeder
/// andere Grund, an dem eine Verbindung scheitert (Rechte, ein fremdes Token,
/// ein Daemon, der nicht antwortet), ist kein „kein Daemon", sondern
/// [`Outcome::Unknown`].
#[must_use]
pub const fn daemon_absent(token: bool, socket: bool) -> bool {
    !token || !socket
}

/// Fragt erst nach der Minor und dann den Daemon selbst.
async fn ask(client: &mut Client) -> Outcome {
    let question = async {
        let info = client.get_info(()).await.ok()?.into_inner();
        if info.proto_minor < RETIRE_MINOR {
            return Some(Outcome::Unsupported);
        }
        let mut events = client
            .sandbox(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::RetireIfIdle(())),
            })
            .await
            .ok()?
            .into_inner();
        while let Ok(Some(event)) = events.message().await {
            if matches!(event.event, Some(v1::sandbox_event::Event::Diagnostic(_))) {
                continue;
            }
            return Some(Outcome::of(event.event.as_ref()));
        }
        None
    };
    tokio::time::timeout(TIMEOUT, question)
        .await
        .ok()
        .flatten()
        .unwrap_or(Outcome::Unknown)
}

#[cfg(test)]
mod tests {
    use super::Outcome;
    use humanitl_ipc::v1;
    use v1::sandbox_event::Event;

    /// Nur ein `Retire` mit `retired` ist `retired`; eine Momentaufnahme
    /// heißt, der Daemon kennt die Bitte nicht, und nichts ist `unknown`.
    #[test]
    fn only_a_retire_event_retires() {
        assert_eq!(
            Outcome::of(Some(&Event::Retire(v1::sandbox_event::Retire {
                retired: true
            }))),
            Outcome::Retired
        );
        assert_eq!(
            Outcome::of(Some(&Event::Retire(v1::sandbox_event::Retire {
                retired: false
            }))),
            Outcome::Busy
        );
        assert_eq!(
            Outcome::of(Some(&Event::Status(v1::sandbox_event::Status::default()))),
            Outcome::Unsupported
        );
        assert_eq!(Outcome::of(None), Outcome::Unknown);
        assert_eq!(
            Outcome::of(Some(&Event::ArgvLine(String::new()))),
            Outcome::Unknown
        );
    }

    /// `none` nur ohne Token oder ohne Socket.
    #[test]
    fn no_daemon_means_no_token_or_no_socket() {
        assert!(super::daemon_absent(false, true), "socket activation, idle");
        assert!(super::daemon_absent(true, false));
        assert!(super::daemon_absent(false, false));
        assert!(
            !super::daemon_absent(true, true),
            "then a failure is unknown"
        );
    }

    /// Die Namen sind der Vertrag mit `postinst` und der App.
    #[test]
    fn the_names_are_the_contract() {
        let names: Vec<&str> = [
            Outcome::Retired,
            Outcome::Busy,
            Outcome::Unsupported,
            Outcome::NoDaemon,
            Outcome::Unknown,
        ]
        .iter()
        .map(|outcome| outcome.name())
        .collect();
        assert_eq!(names, ["retired", "busy", "unsupported", "none", "unknown"]);
    }
}
