//! Was ein Lauf dem Daemon als Wunsch schickt, und was er ihm überlässt
//! (HUM-532).
//!
//! Die Felder `work_mode`, `ask_mode` und `sandbox_mode` eines Starts liegen
//! beim Daemon auf der Ebene der Kommandozeile, also über der Projektwahl.
//! Schickte `humanitl run` sie immer, aus seiner eigenen Auflösung von
//! `config.toml` und den Vorgabewerten, machte es jede Projektwahl dieser
//! Schlüssel wirkungslos: Ein gespeichertes `sandbox.work_mode = "ro"` würde
//! zu `rw`. Ein Wert reist deshalb nur, wenn er ein Wunsch dieses Laufs ist:
//! ein Schalter, eine Variable `HUMANITL_*` des Aufrufs oder eine Datei, die
//! `--config` nennt und die der Daemon nicht liest. Sonst bleibt das Feld
//! leer, und der Daemon löst selbst auf, mit der Projektwahl.

use humanitl_config::{AskMode, Origin, Resolved, SandboxMode};
use humanitl_ipc::client::Client;
use humanitl_ipc::session::ask_mode_name;
use humanitl_ipc::v1;

/// Ab dieser Minor kennt der Daemon `Projects` (HUM-532).
const PROJECTS_MINOR: u32 = 45;

/// Ob der aufgelöste Wert von `key` ein Wunsch dieses Laufs ist.
///
/// `named_config` heißt: `--config` nennt eine Datei; ihre Werte sind dann
/// ein Wunsch, denn der Daemon liest nur seine eigene `config.toml`.
#[must_use]
pub(super) fn per_run(resolved: &Resolved, key: &str, named_config: bool) -> bool {
    match resolved.origin(key) {
        Some(Origin::Cli | Origin::Env(_)) => true,
        Some(Origin::Global) => named_config,
        _ => false,
    }
}

/// `Start.work_mode`: der Wunsch dieses Laufs, sonst leer.
#[must_use]
pub(super) fn work_mode(resolved: &Resolved, named_config: bool) -> String {
    if per_run(resolved, "sandbox.work_mode", named_config) {
        super::work_mode_name(resolved.config.sandbox.work_mode).to_owned()
    } else {
        String::new()
    }
}

/// `Start.ask_mode`: der Wunsch dieses Laufs, sonst leer.
#[must_use]
pub(super) fn ask_mode(resolved: &Resolved, named_config: bool) -> String {
    if per_run(resolved, "hold.ask_mode", named_config) {
        ask_mode_name(resolved.config.hold.ask_mode).to_owned()
    } else {
        String::new()
    }
}

/// Der aufgelöste Modus der Schicht, wenn er ein Wunsch dieses Laufs ist;
/// `--disposable` und `--direct` liest `start_mode` selbst.
#[must_use]
pub(super) fn mode(resolved: &Resolved, named_config: bool) -> Option<SandboxMode> {
    sandbox_mode(false, false, resolved, named_config)
}

/// Der Modus der Schicht als Wunsch: `--disposable`, `--direct`, sonst der
/// aufgelöste Wert nur, wenn er ein Wunsch dieses Laufs ist; sonst `None`.
#[must_use]
pub(super) fn sandbox_mode(
    disposable: bool,
    direct: bool,
    resolved: &Resolved,
    named_config: bool,
) -> Option<SandboxMode> {
    if disposable {
        Some(SandboxMode::Disposable)
    } else if direct {
        Some(SandboxMode::Direct)
    } else if per_run(resolved, "sandbox.mode", named_config) {
        Some(resolved.config.sandbox.mode)
    } else {
        None
    }
}

/// Der Frage-Modus, den die Projektwahl dieses Ordners festlegt, wenn sie es
/// tut: Der Lauf muss wissen, ob er im Terminal fragt. `None` für einen
/// älteren Daemon, ein unbekanntes Projekt oder eine Wahl ohne ihn.
pub(super) async fn chosen_ask_mode(
    client: &mut Client,
    resolved: &Resolved,
    named_config: bool,
    work_dir: &std::path::Path,
    daemon_minor: u32,
) -> Option<AskMode> {
    // Ein Wunsch dieses Laufs geht der Projektwahl vor.
    if daemon_minor < PROJECTS_MINOR || per_run(resolved, "hold.ask_mode", named_config) {
        return None;
    }
    let shown = client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                project_id: String::new(),
                path: work_dir.display().to_string(),
            })),
        })
        .await
        .ok()?
        .into_inner();
    let field = shown
        .choice
        .iter()
        .find(|field| field.key == "hold.ask_mode" && field.origin == "project-choice")?;
    serde_json::from_value(serde_json::Value::String(field.effective.clone())).ok()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{Env, Sources, load};

    use super::*;

    /// Mutationsbeweis: Schickte der Lauf den aufgelösten Modus immer (wie
    /// vor HUM-532), stünde hier `rw`, und die Projektwahl `ro` des Daemons
    /// wäre überstimmt.
    #[test]
    fn without_a_flag_nothing_outranks_the_project_choice() {
        let resolved = load(&Sources::empty()).unwrap();
        assert_eq!(work_mode(&resolved, false), "");
        assert_eq!(ask_mode(&resolved, false), "");
        assert_eq!(sandbox_mode(false, false, &resolved, false), None);
    }

    #[test]
    fn a_flag_and_the_environment_are_wishes_of_this_run() {
        let flagged = load(&Sources::empty().with_cli([("sandbox.work_mode", "ro")])).unwrap();
        assert_eq!(work_mode(&flagged, false), "ro");
        let from_env = load(
            &Sources::empty().with_env(Env::from_pairs([("HUMANITL_HOLD__ASK_MODE", "none")])),
        )
        .unwrap();
        assert_eq!(ask_mode(&from_env, false), "none");
        assert_eq!(
            sandbox_mode(true, false, &from_env, false),
            Some(SandboxMode::Disposable)
        );
    }
}
