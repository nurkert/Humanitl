//! Was der Bildschirm bei den RPCs der besonderen Zeilen abfragt und was er
//! aus ihren Antworten macht (GitHub #7): die Liste der Server, den Stand von
//! Schlüssel und Proxy, die Backends, die Agenten, die Version und die Frage
//! nach Updates. Eine Antwort auf eine Aktion an einer Zeile wird dort zu
//! einer Auskunft in einem Satz.

use humanitl_ipc::v1;

use super::actions::{EGRESS_CHECK, PROXY_AUTH};
use super::link::{Answer, Request};
use super::{Note, SettingsScreen};

/// Fragt die RPCs der besonderen Zeilen ab.
pub fn load_specials(screen: &SettingsScreen) {
    use v1::{agents_request, egress_request, llm_key_request, llm_servers_request};

    screen.send(
        None,
        Request::LlmServers(v1::LlmServersRequest {
            op: Some(llm_servers_request::Op::List(llm_servers_request::List {})),
        }),
    );
    screen.send(
        None,
        Request::LlmKey(v1::LlmKeyRequest {
            op: Some(llm_key_request::Op::Status(llm_key_request::Status {})),
            server: String::new(),
        }),
    );
    let plan = screen
        .project
        .as_ref()
        .map(|scope| v1::sandbox_request::Plan {
            work_dir: scope.path.clone(),
            ..v1::sandbox_request::Plan::default()
        });
    screen.send(
        None,
        Request::SandboxBackends(v1::SandboxBackendsRequest { plan }),
    );
    screen.send(
        None,
        Request::Egress(v1::EgressRequest {
            op: Some(egress_request::Op::Status(egress_request::Status {})),
        }),
    );
    screen.send(
        None,
        Request::Agents(v1::AgentsRequest {
            op: Some(agents_request::Op::Status(agents_request::Status {})),
        }),
    );
    screen.send(None, Request::GetInfo);
    screen.send(None, update_check(false));
}

/// `Update(check)`; `force` fragt jetzt bei GitHub.
pub fn update_check(force: bool) -> Request {
    Request::Update(v1::UpdateRequest {
        op: Some(v1::update_request::Op::Check(v1::update_request::Check {
            force,
            appimage: String::new(),
        })),
    })
}

/// Eine Auskunft und die Befunde, die der Daemon ihr mitgab.
type Found = (String, Vec<v1::Diagnostic>);

/// Nimmt die Antwort einer besonderen Zeile.
pub fn take_special(screen: &mut SettingsScreen, row: Option<String>, answer: Answer) {
    let changed = row.is_some();
    let note: Option<Found> = match answer {
        Answer::Servers(list) => {
            let note =
                (!list.diagnostics.is_empty()).then(|| (String::new(), list.diagnostics.clone()));
            screen.specials.servers = Some(list);
            note
        }
        Answer::Key(status) => {
            let text = if status.set {
                "The access key is stored in the keyring."
            } else {
                "No access key is stored."
            };
            let note = Some((text.to_owned(), status.diagnostics.clone()));
            screen.specials.key = Some(status);
            note
        }
        Answer::Probe(probe) => Some((probe_text(&probe), probe.diagnostics)),
        Answer::Backends(list) => {
            screen.specials.backends = Some(list);
            None
        }
        Answer::Egress(status) => {
            let note =
                egress_text(row.as_deref(), &status).map(|text| (text, status.diagnostics.clone()));
            screen.specials.egress = Some(status);
            note
        }
        Answer::Agents(status) => {
            let diagnostics: Vec<v1::Diagnostic> = status
                .agents
                .iter()
                .flat_map(|agent| agent.diagnostics.iter().cloned())
                .collect();
            screen.specials.agents = Some(status);
            (!diagnostics.is_empty()).then(|| (String::new(), diagnostics))
        }
        Answer::Info(info) => {
            screen.specials.info = Some(info);
            None
        }
        Answer::Update(status) => {
            let note = Some((
                update_text(&status),
                status.notice.iter().cloned().collect(),
            ));
            screen.specials.update = Some(status);
            note
        }
        Answer::Config(_)
        | Answer::Project(_)
        | Answer::Written
        | Answer::Saved(_)
        | Answer::Changed(_)
        | Answer::LiveEnded => None,
    };
    let Some(row) = row else {
        return;
    };
    match note {
        Some((text, diagnostics)) => screen.notes.insert(row, Note::Daemon(text, diagnostics)),
        None => screen.notes.remove(&row),
    };
    if changed {
        screen.load();
    }
}

/// Was eine Probe gemessen hat, als ein Satz; die Befunde stehen darunter.
fn probe_text(probe: &v1::ProbeLlmResponse) -> String {
    if probe.models.is_empty() {
        if probe.answer.is_empty() {
            "No model list came back.".to_owned()
        } else {
            format!("No model list came back: {}.", probe.answer)
        }
    } else {
        format!(
            "{} models in {} ms: {}",
            probe.models.len(),
            probe.latency_ms,
            probe.models.join(", ")
        )
    }
}

/// Was nach einer Antwort von `Egress` an der Zeile steht; die Befunde
/// stehen darunter.
fn egress_text(row: Option<&str>, status: &v1::EgressStatus) -> Option<String> {
    Some(match row? {
        EGRESS_CHECK if status.reachable => {
            format!("Tunnel and TLS handshake to {} stood.", status.check_target)
        }
        EGRESS_CHECK => "The connection did not stand.".to_owned(),
        PROXY_AUTH if status.auth_set => "The credentials are stored.".to_owned(),
        PROXY_AUTH => "No credentials are stored.".to_owned(),
        _ => return None,
    })
}

/// Was der Updater weiß, als ein Satz; der Hinweis steht darunter.
fn update_text(status: &v1::UpdateStatus) -> String {
    if status.available {
        format!(
            "{} is available (this is {}).",
            status.latest_version, status.current_version
        )
    } else if status.latest_version.is_empty() {
        "Nobody has asked for updates yet.".to_owned()
    } else {
        format!("{} is the latest release.", status.current_version)
    }
}
