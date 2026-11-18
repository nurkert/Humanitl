//! Welche Schlüssel die Schreibwege neben `SetConfig` berühren, damit eine
//! Sperre der Firmenvorgabe sie vor jedem Schreiben ablehnt (HUM-534).
//!
//! `SetConfig` prüft seinen einen Schlüssel selbst
//! ([`crate::config_rpc::set_locked`]). `LlmServers`, `LlmKey`, `Egress` und
//! `Agents` schreiben `config.toml` oder den Schlüsselbund auf eigenem Weg;
//! der Dienst fragt für sie [`refuse`], bevor er die Arbeit beginnt. Die
//! Liste ist mit Absicht weit: Ein Server ist Endpunkt, Modelle, Schlüssel und
//! Wahl zugleich, und ist einer davon gesperrt, ändert keine Operation der
//! Server etwas.

use humanitl_config::PolicyLocks;
use humanitl_core::Diagnostic;

use crate::v1;

/// Was eine ändernde Operation von `LlmServers` berühren kann.
pub const SERVER_KEYS: [&str; 6] = [
    "llm.servers",
    "llm.active",
    "llm.server",
    "llm.endpoint",
    "llm.models",
    "llm.api_key_ref",
];

/// Was `LlmKey(set|remove)` berührt: der Verweis, flach oder im Eintrag
/// eines Servers.
pub const KEY_KEYS: [&str; 2] = ["llm.api_key_ref", "llm.servers"];

/// Was `Egress(set_auth|remove_auth)` berührt.
pub const EGRESS_AUTH_KEYS: [&str; 1] = ["egress.proxy_auth_ref"];

/// Die Schlüssel, die eine Anfrage schreiben würde; leer für eine, die nur
/// liest.
#[must_use]
pub fn of_llm_servers(request: &v1::LlmServersRequest) -> &'static [&'static str] {
    match request.op {
        Some(v1::llm_servers_request::Op::List(_)) | None => &[],
        Some(_) => &SERVER_KEYS,
    }
}

/// Wie [`of_llm_servers`] für `LlmKey`.
#[must_use]
pub fn of_llm_key(request: &v1::LlmKeyRequest) -> &'static [&'static str] {
    match request.op {
        Some(v1::llm_key_request::Op::Status(_)) | None => &[],
        Some(_) => &KEY_KEYS,
    }
}

/// Wie [`of_llm_servers`] für `Egress`.
#[must_use]
pub fn of_egress(op: Option<&v1::egress_request::Op>) -> &'static [&'static str] {
    match op {
        Some(v1::egress_request::Op::SetAuth(_) | v1::egress_request::Op::RemoveAuth(_)) => {
            &EGRESS_AUTH_KEYS
        }
        _ => &[],
    }
}

/// Wie [`of_llm_servers`] für `Agents`: `agents.source` und
/// `agents.<id>.source` des genannten Agenten.
#[must_use]
pub fn of_agents(op: Option<&v1::agents_request::Op>) -> Vec<String> {
    match op {
        Some(v1::agents_request::Op::SetSource(set)) => vec![
            "agents.source".to_owned(),
            format!("agents.{}.source", set.agent.trim()),
        ],
        _ => Vec::new(),
    }
}

/// Lehnt ab, wenn einer der Schlüssel gesperrt ist.
///
/// # Errors
///
/// `AGENT_010` für den ersten gesperrten Schlüssel.
pub fn refuse<I, K>(keys: I, locks: &PolicyLocks) -> Result<(), Diagnostic>
where
    I: IntoIterator<Item = K>,
    K: AsRef<str>,
{
    humanitl_config::policy::refuse_locked_keys(keys, locks)
}
