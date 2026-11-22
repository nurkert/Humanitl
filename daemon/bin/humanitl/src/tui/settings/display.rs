//! Der Wert einer Zeile, wie die Liste ihn zeigt (GitHub #7): nach der
//! Bedienart des Katalogs, bei den besonderen Zeilen aus ihren eigenen RPCs.
//! Ein Geheimnis erscheint nie, nur ob es hinterlegt ist.

use humanitl_ipc::v1;

use super::actions::{
    AGENTS, API_KEY, BACKEND, CONFIG_FILE, EGRESS_CHECK, PROXY_AUTH, RESTART, SERVER_USE, SERVERS,
    VERSION,
};
use super::model;
use super::{SettingsScreen, Specials};

/// Der Wert einer Zeile, wie die Liste ihn zeigt; nie ein Geheimnis.
#[must_use]
pub fn shown_value(screen: &SettingsScreen, entry: &v1::SettingsEntry) -> String {
    let value = model::value(&screen.values, &entry.id);
    let text = match entry.control.as_str() {
        "secret_ref" => secret_state(screen, &entry.id).to_owned(),
        "switch" => if model::is_on(value) { "on" } else { "off" }.to_owned(),
        "choice" => value
            .map(|value| model::choice_title(entry, &model::as_text(value)))
            .unwrap_or_default(),
        "special" => special_value(screen, entry),
        _ => value.map(model::as_text).unwrap_or_default(),
    };
    if text.is_empty() {
        "not set".to_owned()
    } else {
        model::clean(&text)
    }
}

/// Ob ein Geheimnis hinterlegt ist; das Geheimnis selbst kennt der Client
/// nie.
fn secret_state(screen: &SettingsScreen, id: &str) -> &'static str {
    let stored = match id {
        API_KEY => screen.specials.key.as_ref().map(|key| key.set),
        PROXY_AUTH => screen
            .specials
            .egress
            .as_ref()
            .map(|egress| egress.auth_set),
        _ => None,
    };
    match stored {
        Some(true) => "stored",
        Some(false) => "not stored",
        None => "",
    }
}

/// Der Wert einer besonderen Zeile.
fn special_value(screen: &SettingsScreen, entry: &v1::SettingsEntry) -> String {
    let specials = &screen.specials;
    match entry.id.as_str() {
        SERVERS => specials.servers.as_ref().map_or_else(String::new, |list| {
            if list.servers.is_empty() {
                "none saved".to_owned()
            } else {
                format!("{} saved, default {}", list.servers.len(), list.active)
            }
        }),
        SERVER_USE if screen.project.is_some() => model::value(&screen.values, &entry.id)
            .map_or_else(|| "default".to_owned(), model::as_text),
        SERVER_USE => "Enter shows who uses which server".to_owned(),
        AGENTS => specials.agents.as_ref().map_or_else(String::new, |status| {
            status
                .agents
                .iter()
                .map(|agent| format!("{}: {}", agent.agent, agent.source))
                .collect::<Vec<_>>()
                .join(", ")
        }),
        BACKEND => {
            let written = model::value(&screen.values, &entry.id)
                .map(model::as_text)
                .unwrap_or_default();
            let auto = specials
                .backends
                .as_ref()
                .map(|list| list.auto_backend.clone())
                .unwrap_or_default();
            if written.is_empty() || written == "auto" {
                format!("Automatic ({auto})")
            } else {
                written
            }
        }
        EGRESS_CHECK => "Enter checks the connection".to_owned(),
        VERSION => version_text(specials),
        RESTART => "Enter restarts".to_owned(),
        CONFIG_FILE => screen.config_path.clone(),
        _ => model::value(&screen.values, &entry.id)
            .map(model::as_text)
            .unwrap_or_default(),
    }
}

/// Die Version des Daemons und, wenn bekannt, ein neueres Release.
fn version_text(specials: &Specials) -> String {
    let version = specials
        .info
        .as_ref()
        .map(|info| info.daemon_version.clone())
        .unwrap_or_default();
    match specials.update.as_ref() {
        Some(update) if update.available => {
            format!("{version} · {} available", update.latest_version)
        }
        _ => version,
    }
}
