//! Ob im Daemon gerade eine Agenten-Sitzung läuft (HUM-284).
//!
//! `humanitl daemon status` sagt es in der Zeile `agent session` und unter
//! `--json` im Feld `agent_session`. Gebraucht wird die Antwort vor einem
//! Neustart des Daemons nach einem Paket-Update: `postinst` startet den
//! Dienst eines Menschen nur neu, wenn hier `none` steht
//! (`packaging/deb/maintscript-users.sh`). Jede andere Antwort heißt dort:
//! nicht neu starten. Deshalb ist eine Frage, die nicht durchkommt, nie
//! `none`, sondern `unknown`.
//!
//! Die Kennung `session_id` aus `GetInfo` beantwortet die Frage nicht: Sie
//! nennt die Proxy-Sitzung, die der Daemon bei seinem Start anlegt, und steht
//! bei jedem laufenden Daemon da.

use std::time::Duration;

use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

/// Wie lange `daemon status` auf die Momentaufnahme aus `Sandbox(Status)`
/// wartet.
///
/// Kurz, weil sie nur eine Momentaufnahme ist; wer sie nicht bekommt, liest
/// `unknown`.
const TIMEOUT: Duration = Duration::from_secs(5);

/// Die Antwort auf die Frage, ob eine Agenten-Sitzung läuft.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSession {
    /// Keine Sitzung, auch keine, die gerade startet oder endet.
    None,
    /// Eine Sitzung läuft, startet oder endet, oder ihr Agent hat sich beendet
    /// und sie ist noch nicht gestoppt.
    Running,
    /// Der Daemon hat nicht oder nicht lesbar geantwortet.
    Unknown,
}

impl AgentSession {
    /// Der Wert in `daemon status`: `none`, `running` oder `unknown`.
    ///
    /// Die Namen sind Vertrag: `packaging/deb/maintscript-users.sh` sucht
    /// `"agent_session":"none"` in der JSON-Zeile.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Running => "running",
            Self::Unknown => "unknown",
        }
    }

    /// Was eine Momentaufnahme aus `Sandbox(Status)` über die Sitzung sagt.
    ///
    /// `failed` mit einer Kennung ist eine gehaltene Sandbox, deren Plan
    /// gerade nicht aufgeht; die Sitzung läuft trotzdem. `failed` ohne
    /// Kennung ist ein Plan, der ohne Sitzung nicht aufgeht.
    #[must_use]
    pub fn of(status: &v1::sandbox_event::Status) -> Self {
        match v1::SandboxState::try_from(status.state) {
            Ok(v1::SandboxState::Stopped) => Self::None,
            Ok(
                v1::SandboxState::Starting | v1::SandboxState::Running | v1::SandboxState::Stopping,
            ) => Self::Running,
            Ok(v1::SandboxState::Failed) => {
                if status.sandbox_id.is_empty() {
                    Self::None
                } else {
                    Self::Running
                }
            }
            Ok(v1::SandboxState::Unspecified) | Err(_) => Self::Unknown,
        }
    }
}

/// Fragt `Sandbox(Status)` und liest die erste Momentaufnahme.
///
/// Befunde vor ihr werden übergangen. Ein Strom ohne Momentaufnahme, ein
/// Fehler oder keine Antwort binnen [`TIMEOUT`] ist
/// [`AgentSession::Unknown`].
pub async fn ask(client: &mut Client) -> AgentSession {
    let question = async {
        let mut events = client
            .sandbox(v1::SandboxRequest {
                op: Some(v1::sandbox_request::Op::Status(())),
            })
            .await
            .ok()?
            .into_inner();
        while let Ok(Some(event)) = events.message().await {
            if let Some(v1::sandbox_event::Event::Status(status)) = event.event {
                return Some(AgentSession::of(&status));
            }
        }
        None
    };
    tokio::time::timeout(TIMEOUT, question)
        .await
        .ok()
        .flatten()
        .unwrap_or(AgentSession::Unknown)
}

#[cfg(test)]
mod tests {
    use super::AgentSession;
    use humanitl_ipc::v1;

    fn status(state: v1::SandboxState, sandbox_id: &str) -> v1::sandbox_event::Status {
        v1::sandbox_event::Status {
            state: state.into(),
            sandbox_id: sandbox_id.to_owned(),
            ..v1::sandbox_event::Status::default()
        }
    }

    /// Nur ein gestoppter Dienst und ein Plan ohne Sitzung sind `none`; alles,
    /// was eine Sitzung hält, ist `running`, und ein Zustand, den diese
    /// Kommandozeile nicht kennt, `unknown`.
    #[test]
    fn only_a_stopped_sandbox_is_no_session() {
        use v1::SandboxState as S;
        assert_eq!(
            AgentSession::of(&status(S::Stopped, "")),
            AgentSession::None
        );
        for held in [S::Starting, S::Running, S::Stopping] {
            assert_eq!(
                AgentSession::of(&status(held, "s-1")),
                AgentSession::Running,
                "{held:?}"
            );
        }
        assert_eq!(
            AgentSession::of(&status(S::Failed, "s-1")),
            AgentSession::Running,
            "a held sandbox whose plan fails right now"
        );
        assert_eq!(
            AgentSession::of(&status(S::Failed, "")),
            AgentSession::None,
            "a plan that fails without a session"
        );
        assert_eq!(
            AgentSession::of(&status(S::Unspecified, "")),
            AgentSession::Unknown
        );
        let mut future = status(S::Stopped, "");
        future.state = 99;
        assert_eq!(AgentSession::of(&future), AgentSession::Unknown);
    }

    /// Die Namen sind der Vertrag mit `postinst`.
    #[test]
    fn the_names_are_the_contract_with_postinst() {
        assert_eq!(AgentSession::None.name(), "none");
        assert_eq!(AgentSession::Running.name(), "running");
        assert_eq!(AgentSession::Unknown.name(), "unknown");
    }
}
