//! Was eine Zeile tut, wenn sie bedient wird, und was sie zeigt
//! (GitHub #7).
//!
//! Die Bedienart kommt aus dem Katalog (`control`): Schalter, Auswahl, Text,
//! Zahl, Pfad, Geheimnis. Die Zeilen `special` haben ein Bedienelement, das
//! über ihre Kennung zugeordnet ist, wie die App es über ihre Kennungen tut;
//! die Kennungen hier sind Namen des Katalogs, keine eigene Liste von
//! Einstellungen. Eine `special`-Zeile, die dieses Modul nicht kennt, steht
//! trotzdem da, mit Wert, Herkunft und Hilfe.

use humanitl_ipc::v1;

use super::edit::{Confirm, Input, Opt, Overlay, Picker, Target};
use super::link::Request;
use super::model;
use super::panels::Panel;
use super::{Exit, Note, ProjectScope, SettingsScreen};

/// Die Modellserver (`LlmServers`).
pub const SERVERS: &str = "llm.servers";
/// Wer welchen Server nutzt (`LlmServers.Assign`).
pub const SERVER_USE: &str = "llm.server";
/// Der Zugangsschlüssel des aktiven Servers (`LlmKey`).
pub const API_KEY: &str = "llm.api_key_ref";
/// Das Agent-Depot (`Agents`).
pub const AGENTS: &str = "agents.depot";
/// Das Sandbox-Backend (`SandboxBackends`).
pub const BACKEND: &str = "sandbox.backend";
/// Die Zugangsdaten des Entscheidungs-Proxys (`Egress`).
pub const PROXY_AUTH: &str = "egress.proxy_auth_ref";
/// Die Prüfung des Entscheidungs-Proxys (`Egress.check_connection`).
pub const EGRESS_CHECK: &str = "egress.check";
/// Die Version des Daemons (`GetInfo`) und die Frage nach Updates
/// (`Update`).
pub const VERSION: &str = "daemon.version";
/// Der Neustart des Daemons.
pub const RESTART: &str = "daemon.restart";
/// Der Pfad von `config.toml` (`ConfigSnapshot.config_path`).
pub const CONFIG_FILE: &str = "daemon.config_file";

/// Der Katalogeintrag mit dieser Kennung.
fn entry(screen: &SettingsScreen, id: &str) -> Option<v1::SettingsEntry> {
    screen
        .catalog
        .as_ref()?
        .entries
        .iter()
        .find(|entry| entry.id == id)
        .cloned()
}

/// Warum eine Zeile nur lesbar ist, oder `None`, wenn sie sich ändern lässt.
///
/// Eine Sperre der Firmenvorgabe gewinnt immer. Mit Projekt lässt sich nur
/// ändern, was zur Projektwahl gehört; alles andere schriebe die globale
/// Ebene, und das ist nicht diese Ansicht.
#[must_use]
pub fn read_only(screen: &SettingsScreen, entry: &v1::SettingsEntry) -> Option<String> {
    if let Some(lock) = entry.lock.as_ref() {
        let mut text = format!("Locked by {}, read only.", lock.locked_by);
        if !lock.reason.is_empty() {
            text.push(' ');
            text.push_str(&lock.reason);
        }
        return Some(text);
    }
    let scope = screen.project.as_ref()?;
    if !entry.in_project_choice {
        return Some(
            "Not part of the project choice, so read only here; without --project it is a \
             global setting."
                .to_owned(),
        );
    }
    if !scope.loaded {
        let mut text = "The project choice of this folder has not loaded yet; nothing is \
                        written before it has."
            .to_owned();
        if let Some(super::Note::Refused(found)) = screen.notes.get(super::PROJECT_ROW) {
            text.push(' ');
            text.push_str(&found.why);
        }
        return Some(text);
    }
    if scope.saving.is_some() {
        return Some("A change of the project choice is still on its way; wait for it.".to_owned());
    }
    None
}

/// Bedient die Zeile `id`: `Enter` oder Leertaste.
pub fn activate(screen: &mut SettingsScreen, id: &str) {
    let Some(entry) = entry(screen, id) else {
        return;
    };
    // Nur lesbar: Der Grund steht schon unter der Zeile; es geschieht nichts.
    if read_only(screen, &entry).is_some() {
        return;
    }
    let current = current_text(screen, &entry);
    match entry.control.as_str() {
        "switch" => {
            let on = model::is_on(model::value(&screen.values, &entry.id));
            write(screen, &entry.id, (!on).to_string());
        }
        "choice" => choose(screen, &entry, &current),
        "text" | "number" | "path" => {
            screen.overlay = Some(Overlay::Input(Input::plain(
                model::clean(&entry.title),
                current,
                Target::Write(entry.id.clone()),
            )));
        }
        "secret_ref" => secret(screen, &entry),
        "special" => special(screen, &entry, &current),
        _ => no_control(screen, &entry),
    }
}

/// Der Wert einer Zeile als Text, wie `SetConfig` ihn nähme.
fn current_text(screen: &SettingsScreen, entry: &v1::SettingsEntry) -> String {
    model::value(&screen.values, &entry.id)
        .map(model::as_text)
        .unwrap_or_default()
}

/// Eine Auswahl aus den Werten des Katalogs.
fn choose(screen: &mut SettingsScreen, entry: &v1::SettingsEntry, current: &str) {
    if entry.choices.is_empty() {
        no_control(screen, entry);
        return;
    }
    let options = entry
        .choices
        .iter()
        .map(|choice| Opt::new(choice.value.clone(), model::clean(&choice.title)))
        .collect();
    screen.overlay = Some(Overlay::Picker(Picker::new(
        model::clean(&entry.title),
        options,
        current,
        Target::Write(entry.id.clone()),
    )));
}

/// Ein Geheimnis: verdeckt getippt, über seine eigene RPC.
fn secret(screen: &mut SettingsScreen, entry: &v1::SettingsEntry) {
    let target = match entry.id.as_str() {
        API_KEY => Target::LlmKey(active_server(screen)),
        PROXY_AUTH => Target::EgressAuth,
        // Ein Geheimnis ohne eigene RPC geht nie über `SetConfig`.
        _ => {
            no_control(screen, entry);
            return;
        }
    };
    let label = match target {
        Target::EgressAuth => format!("{} (USER:PASSWORD, hidden)", model::clean(&entry.title)),
        _ => format!("{} (hidden)", model::clean(&entry.title)),
    };
    screen.overlay = Some(Overlay::Input(Input::secret(label, target)));
}

/// Der aktive Modellserver; leer ohne gespeicherte Server.
fn active_server(screen: &SettingsScreen) -> String {
    screen
        .specials
        .servers
        .as_ref()
        .filter(|list| !list.implicit)
        .map(|list| list.active.clone())
        .unwrap_or_default()
}

/// Eine Zeile ohne Bedienelement im Terminal.
fn no_control(screen: &mut SettingsScreen, entry: &v1::SettingsEntry) {
    screen.notes.insert(
        entry.id.clone(),
        Note::Info(format!(
            "No control for this row in the terminal; humanitl config set {} <value> changes it.",
            entry.id
        )),
    );
}

/// Die besonderen Zeilen, nach Kennung.
fn special(screen: &mut SettingsScreen, entry: &v1::SettingsEntry, current: &str) {
    match entry.id.as_str() {
        SERVERS => screen.panel = Some(Panel::Servers { cursor: 0 }),
        SERVER_USE if screen.project.is_some() => {
            let options = server_options(screen);
            screen.overlay = Some(Overlay::Picker(Picker::new(
                model::clean(&entry.title),
                options,
                current,
                Target::Write(entry.id.clone()),
            )));
        }
        SERVER_USE => screen.panel = Some(Panel::Usage { cursor: 0 }),
        AGENTS => screen.panel = Some(Panel::Agents { cursor: 0 }),
        BACKEND => backend(screen, entry, current),
        EGRESS_CHECK => {
            screen.notes.insert(
                entry.id.clone(),
                Note::Info("Checking the connection…".to_owned()),
            );
            screen.send(
                Some(EGRESS_CHECK),
                Request::Egress(v1::EgressRequest {
                    op: Some(v1::egress_request::Op::CheckConnection(
                        v1::egress_request::CheckConnection {},
                    )),
                }),
            );
        }
        VERSION => {
            screen.notes.insert(
                entry.id.clone(),
                Note::Info("Asking for a newer release…".to_owned()),
            );
            screen.send(Some(VERSION), super::replies::update_check(true));
        }
        RESTART => {
            screen.overlay = Some(Overlay::Confirm(Confirm {
                question: "Restart the background service now? The daemon refuses while an \
                           agent session holds it."
                    .to_owned(),
                target: Target::Restart,
            }));
        }
        CONFIG_FILE if !screen.config_path.is_empty() => {
            screen.exit = Some(Exit::Editor(screen.config_path.clone().into()));
        }
        _ => choose(screen, entry, current),
    }
}

/// Die gespeicherten Server als Auswahl, mit „Standard" (leer) zuerst.
#[must_use]
pub fn server_options(screen: &SettingsScreen) -> Vec<Opt> {
    let mut options = vec![Opt::new("", "Default server")];
    if let Some(list) = screen.specials.servers.as_ref() {
        options.extend(
            list.servers
                .iter()
                .map(|server| Opt::new(server.name.clone(), model::clean(&server.name))),
        );
    }
    options
}

/// Die Auswahl des Backends aus `SandboxBackends`: automatisch zuerst, dann
/// jedes zugelassene Backend, eines, das hier nicht starten kann, mit dem
/// Grund des Daemons und nicht wählbar (wie in der App).
fn backend(screen: &mut SettingsScreen, entry: &v1::SettingsEntry, current: &str) {
    let Some(list) = screen.specials.backends.as_ref() else {
        screen.notes.insert(
            entry.id.clone(),
            Note::Info("The daemon has not listed its backends yet.".to_owned()),
        );
        return;
    };
    let admitted: Vec<&v1::SandboxBackend> = list
        .backends
        .iter()
        .filter(|backend| {
            !backend
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SANDBOX_048")
        })
        .collect();
    let auto_available = list
        .backends
        .iter()
        .any(|backend| backend.name == list.auto_backend && backend.available);
    let mut auto = Opt::new(
        "auto",
        format!("Automatic ({})", model::clean(&list.auto_backend)),
    );
    auto.enabled = auto_available;
    let mut options = vec![auto];
    for backend in admitted {
        let mut option = Opt::new(backend.name.clone(), model::clean(&backend.name));
        option.enabled = backend.available;
        option.reason = backend
            .diagnostics
            .iter()
            .map(|diagnostic| model::clean(&diagnostic.why))
            .collect::<Vec<_>>()
            .join(" ");
        options.push(option);
    }
    let current = if current.is_empty() { "auto" } else { current };
    screen.overlay = Some(Overlay::Picker(Picker::new(
        model::clean(&entry.title),
        options,
        current,
        Target::Write(entry.id.clone()),
    )));
}

/// Entfernt ein Geheimnis (`Delete` oder `x` an seiner Zeile), nach
/// Rückfrage.
pub fn remove(screen: &mut SettingsScreen, id: &str) {
    let Some(entry) = entry(screen, id) else {
        return;
    };
    if read_only(screen, &entry).is_some() {
        return;
    }
    let (question, target) = match id {
        API_KEY => (
            "Remove the access key from the keyring?".to_owned(),
            Target::RemoveLlmKey(active_server(screen)),
        ),
        PROXY_AUTH => (
            "Remove the proxy credentials from the keyring?".to_owned(),
            Target::RemoveEgressAuth,
        ),
        _ => return,
    };
    screen.overlay = Some(Overlay::Confirm(Confirm { question, target }));
}

/// Schreibt den Wert einer Zeile: global über `SetConfig`, mit Projekt über
/// `Projects.save_choice`.
pub fn write(screen: &mut SettingsScreen, id: &str, value: String) {
    let request = match screen.project.as_mut() {
        Some(scope) => {
            // Nie auf einer Wahl, die noch nicht da ist, und nie zwei
            // zugleich: `save_choice` ersetzt die ganze Wahl.
            if !scope.loaded || scope.saving.is_some() {
                return;
            }
            scope.saving = Some(id.to_owned());
            Request::Projects(save_choice(scope, id, value))
        }
        None => Request::SetConfig(v1::SetConfigRequest {
            key: id.to_owned(),
            value,
            expected_server: expected_server(screen, id),
        }),
    };
    screen.send(Some(id), request);
}

/// Der Server, dessen Ansicht eine Zeile wie `llm.endpoint` bearbeitet: der
/// aktive, damit der Daemon einen Wert nie in einen anderen schreibt, der
/// inzwischen aktiv ist (`CONFIG_021`).
fn expected_server(screen: &SettingsScreen, id: &str) -> String {
    if humanitl_config::servers::is_server_setting(id) {
        active_server(screen)
    } else {
        String::new()
    }
}

/// `save_choice` ersetzt die ganze Projektwahl: die gespeicherten Werte, mit
/// `id` neu oder ersetzt; ein leerer Wert nimmt den Schlüssel heraus.
fn save_choice(scope: &ProjectScope, id: &str, value: String) -> v1::ProjectsRequest {
    let mut values: Vec<v1::ChoiceValue> = scope
        .chosen
        .iter()
        .filter(|(key, _)| key != id)
        .map(|(key, value)| v1::ChoiceValue {
            key: key.clone(),
            value: value.clone(),
        })
        .collect();
    if !value.is_empty() {
        values.push(v1::ChoiceValue {
            key: id.to_owned(),
            value,
        });
    }
    let (project_id, path) = if scope.project_id.is_empty() {
        (String::new(), scope.path.clone())
    } else {
        (scope.project_id.clone(), String::new())
    };
    v1::ProjectsRequest {
        op: Some(v1::projects_request::Op::SaveChoice(
            v1::projects_request::SaveChoice {
                project_id,
                values,
                path,
            },
        )),
    }
}

/// Führt aus, was eine Eingabe, Wahl oder Bestätigung abgeschlossen hat.
///
/// Der Text bleibt unter `Zeroizing`, bis die Anfrage gebaut wird; dorthin
/// zieht er ohne Kopie, und das `Zeroizing` bleibt leer zurück. Das Feld der
/// Anfrage wischt danach niemand: tonic nimmt die Nachricht.
pub fn commit(screen: &mut SettingsScreen, target: Target, mut text: zeroize::Zeroizing<String>) {
    use v1::{egress_request, llm_key_request};

    match target {
        Target::Write(id) => write(screen, &id, moved(&mut text)),
        Target::LlmKey(server) => {
            if text.is_empty() {
                return;
            }
            let row = key_row(screen);
            screen.send(
                Some(row),
                Request::LlmKey(v1::LlmKeyRequest {
                    op: Some(llm_key_request::Op::Set(llm_key_request::Set {
                        key: moved(&mut text),
                    })),
                    server,
                }),
            );
        }
        Target::RemoveLlmKey(server) => {
            let row = key_row(screen);
            screen.send(
                Some(row),
                Request::LlmKey(v1::LlmKeyRequest {
                    op: Some(llm_key_request::Op::Remove(llm_key_request::Remove {})),
                    server,
                }),
            );
        }
        Target::EgressAuth => {
            if text.is_empty() {
                return;
            }
            screen.send(
                Some(PROXY_AUTH),
                Request::Egress(v1::EgressRequest {
                    op: Some(egress_request::Op::SetAuth(egress_request::SetAuth {
                        credentials: moved(&mut text),
                    })),
                }),
            );
        }
        Target::RemoveEgressAuth => screen.send(
            Some(PROXY_AUTH),
            Request::Egress(v1::EgressRequest {
                op: Some(egress_request::Op::RemoveAuth(
                    egress_request::RemoveAuth {},
                )),
            }),
        ),
        Target::Restart => screen.exit = Some(Exit::Restart),
        other => super::panels::commit(screen, other, moved(&mut text)),
    }
}

/// Nimmt den Text aus dem `Zeroizing`, ohne ihn zu kopieren; das
/// `Zeroizing` bleibt leer zurück.
fn moved(text: &mut zeroize::Zeroizing<String>) -> String {
    std::mem::take(&mut **text)
}

/// Die Zeile, an der das Ergebnis einer Aktion am Schlüssel steht: die
/// Seite der Server, wenn sie offen ist, sonst die Zeile des Schlüssels.
fn key_row(screen: &SettingsScreen) -> &'static str {
    if screen.panel.is_some() {
        SERVERS
    } else {
        API_KEY
    }
}
