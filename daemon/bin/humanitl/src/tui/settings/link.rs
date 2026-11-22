//! Die Leitung zwischen dem Bildschirm der Einstellungen und dem Daemon
//! (GitHub #7).
//!
//! Der Bildschirm ruft keine RPC selbst. Er legt einen [`Call`] in die
//! Leitung und liest später eine [`Reply`] heraus; dazwischen arbeitet im
//! Produkt ein Hintergrundauftrag der Laufzeit (`cmd::settings`), im Test ein
//! Fake, der dieselben Nachrichten der Wire-Form beantwortet. So bleibt die
//! Schleife des Bildschirms frei, solange der Daemon rechnet, und ein Test
//! sieht genau die Anfragen, die auf die Leitung gingen.
//!
//! Die Anfragen sind die Nachrichten der Wire-Form selbst, nicht eine eigene
//! Übersetzung: Was der Test als `SetConfig` mit Schlüssel und Wert prüft,
//! ist byteweise das, was der Daemon bekommt.
//!
//! # Geheimnisse
//!
//! `LlmKey(Set)` und `Egress(SetAuth)` tragen ein Geheimnis. Darum hat
//! [`Request`] absichtlich kein `Debug`: Die Ableitung von prost schriebe das
//! Feld im Klartext in jede Ausgabe mit `{:?}`.

use std::sync::mpsc;

use humanitl_core::Diagnostic;
use humanitl_ipc::v1;

/// Eine Anfrage an den Daemon, als Nachricht der Wire-Form.
pub enum Request {
    /// `GetConfig`, im Bildschirm immer mit Katalog.
    GetConfig(v1::GetConfigRequest),
    /// `SetConfig`: genau ein Schlüssel der globalen Ebene.
    SetConfig(v1::SetConfigRequest),
    /// `Projects`: `get` und `save_choice` der Projektebene.
    Projects(v1::ProjectsRequest),
    /// `LlmServers`.
    LlmServers(v1::LlmServersRequest),
    /// `LlmKey`; `Set` trägt den Schlüssel.
    LlmKey(v1::LlmKeyRequest),
    /// `ProbeLlm`.
    ProbeLlm(v1::ProbeLlmRequest),
    /// `SandboxBackends`.
    SandboxBackends(v1::SandboxBackendsRequest),
    /// `Egress`; `SetAuth` trägt die Zugangsdaten.
    Egress(v1::EgressRequest),
    /// `Agents`.
    Agents(v1::AgentsRequest),
    /// `GetInfo`.
    GetInfo,
    /// `Update`; der Bildschirm liest nur den ersten Stand.
    Update(v1::UpdateRequest),
}

impl Request {
    /// Der Name der RPC, für den Befund, wenn sie scheitert.
    #[must_use]
    pub const fn rpc(&self) -> &'static str {
        match self {
            Self::GetConfig(_) => "GetConfig",
            Self::SetConfig(_) => "SetConfig",
            Self::Projects(_) => "Projects",
            Self::LlmServers(_) => "LlmServers",
            Self::LlmKey(_) => "LlmKey",
            Self::ProbeLlm(_) => "ProbeLlm",
            Self::SandboxBackends(_) => "SandboxBackends",
            Self::Egress(_) => "Egress",
            Self::Agents(_) => "Agents",
            Self::GetInfo => "GetInfo",
            Self::Update(_) => "Update",
        }
    }
}

/// Eine Anfrage und die Zeile, zu der ihre Antwort gehört.
pub struct Call {
    /// Die Kennung des Katalogeintrags, an dem eine Ablehnung steht; `None`
    /// für das Laden.
    pub row: Option<String>,
    /// Die Anfrage.
    pub request: Request,
}

/// Was der Daemon geantwortet hat.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "one answer at a time crosses the link; boxing every variant buys nothing"
)]
pub enum Answer {
    /// `GetConfig`: Werte, Herkunft und Katalog.
    Config(v1::ConfigSnapshot),
    /// `Projects(get)`: der Eintrag und die Projektwahl.
    Project(v1::ProjectsResponse),
    /// `SetConfig` hat geschrieben.
    Written,
    /// `Projects(save_choice)` hat geschrieben; die Antwort trägt die
    /// gespeicherte Wahl.
    Saved(v1::ProjectsResponse),
    /// `LlmServers`.
    Servers(v1::LlmServerList),
    /// `LlmKey`, ohne den Schlüssel.
    Key(v1::LlmKeyStatus),
    /// `ProbeLlm`.
    Probe(v1::ProbeLlmResponse),
    /// `SandboxBackends`.
    Backends(v1::SandboxBackendList),
    /// `Egress`, ohne Zugangsdaten.
    Egress(v1::EgressStatus),
    /// `Agents`.
    Agents(v1::AgentsStatus),
    /// `GetInfo`.
    Info(v1::Info),
    /// Der erste Stand von `Update`.
    Update(v1::UpdateStatus),
    /// `FlowEvent.config_changed` aus dem Strom: Ein anderer Client oder die
    /// Datei hat etwas geändert.
    Changed(v1::flow_event::ConfigChanged),
    /// Der Ereignisstrom ist zu Ende oder kam nicht zustande: Änderungen
    /// anderer Clients erscheinen erst nach `r`.
    LiveEnded,
}

/// Eine Antwort und die Zeile ihrer Anfrage.
#[derive(Debug, Clone)]
pub struct Reply {
    /// Wie [`Call::row`].
    pub row: Option<String>,
    /// Die Antwort oder der Befund, mit `why` und, wo es einen gibt, `fix`.
    pub result: Result<Answer, Diagnostic>,
}

/// Die Enden der Leitung, die der Bildschirm hält.
pub struct Link {
    /// Hinaus: die Anfragen.
    calls: tokio::sync::mpsc::UnboundedSender<Call>,
    /// Herein: die Antworten.
    replies: mpsc::Receiver<Reply>,
}

/// Die Enden, die die Gegenseite hält: der Hintergrundauftrag oder der Fake.
pub struct Far {
    /// Die Anfragen des Bildschirms.
    pub calls: tokio::sync::mpsc::UnboundedReceiver<Call>,
    /// Wohin die Antworten gehen.
    pub replies: mpsc::Sender<Reply>,
}

/// Eine neue Leitung.
#[must_use]
pub fn pair() -> (Link, Far) {
    let (call_tx, call_rx) = tokio::sync::mpsc::unbounded_channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    (
        Link {
            calls: call_tx,
            replies: reply_rx,
        },
        Far {
            calls: call_rx,
            replies: reply_tx,
        },
    )
}

impl Link {
    /// Legt eine Anfrage in die Leitung. `false`, wenn die Gegenseite nicht
    /// mehr da ist.
    pub fn send(&self, call: Call) -> bool {
        self.calls.send(call).is_ok()
    }

    /// Die nächste Antwort, ohne zu warten.
    pub fn try_reply(&self) -> Option<Reply> {
        self.replies.try_recv().ok()
    }
}
