//! `LlmServers`: die gespeicherten Modellserver (HUM-289).
//!
//! Sechs Operationen über `[llm.servers.<name>]` in `config.toml`: auflisten,
//! anlegen, umbenennen, entfernen, aktivieren und, seit HUM-405, Endpunkt
//! und Modelle eines Servers ändern, ohne ihn zum Standard zu machen. Dazu
//! zwei, die `config.toml` nicht berühren: einem Profil der Sitzung oder
//! einem Projekt einen Server zuordnen, und die Wahl eines Projekts für
//! seinen Ordner bestätigen ([`humanitl_config::assign`]). Was sich an der Datei ändert,
//! rechnet [`humanitl_config::servers`] aus; geschrieben wird in einem Zug
//! über [`humanitl_config::edit::edit_with`], mit derselben Gegenprobe wie
//! bei `SetConfig` (die Nebendatei muss laden, wie der nächste Start lädt).
//!
//! Keine Antwort trägt einen Schlüssel, nur den Verweis darauf. Den
//! Schlüssel eines Servers setzt `LlmKey` mit dessen Namen
//! ([`crate::llm_key`]). Beim Entfernen löscht der Dienst auf Wunsch den
//! Eintrag im Schlüsselbund, und zwar erst, nachdem die Datei geschrieben ist:
//! Scheitert das Schreiben, bleibt der Server samt Schlüssel, wie er war;
//! scheitert das Löschen, ist der Server weg und die Antwort sagt, welcher
//! Eintrag stehen blieb.
//!
//! Ein Wechsel des aktiven Servers gilt ab der nächsten Sitzung: Jeder Start
//! löst die Konfiguration neu auf (HUM-277), eine laufende Sitzung behält,
//! womit sie gestartet ist.

use humanitl_config::Paths;
use humanitl_config::edit::Edit;
use humanitl_config::secret::{KeyRef, KeyStore};
use humanitl_config::servers::{self, ServerList};
use humanitl_core::diagnostics::codes::{CONFIG_001, CONFIG_021};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::{convert, v1};

/// Was ein Auftrag an der Datei ändert und was danach zu tun bleibt.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// Die Änderungen für [`humanitl_config::edit::edit_with`].
    pub edits: Vec<Edit>,
    /// Der Schlüssel, der nach dem Schreiben gelöscht werden soll: beim
    /// Entfernen mit `delete_key` und beim Bearbeiten, das die Herkunft des
    /// Endpunkts ändert, und nur, wenn kein anderer Server ihn trägt.
    pub delete_key: Option<KeyRef>,
    /// Die Servernamen, deren Bestätigungen in Projekten nach dem Schreiben
    /// wegfallen (HUM-405): Ein Name, der danach einen anderen Server meint,
    /// erbt keine Bestätigung.
    pub forget: Vec<String>,
}

/// Rechnet aus, was ein Auftrag an der Tabelle `table` ändert.
///
/// Der Fake nimmt denselben Weg über seiner Tabelle im Speicher; so lehnen
/// beide Dienste denselben Auftrag mit demselben Befund ab.
///
/// # Errors
///
/// `CONFIG_021` ohne Operation und für alles, was
/// [`humanitl_config::servers`] ablehnt; `CONFIG_003` für einen Endpunkt, der
/// keiner ist.
pub fn plan(table: &toml::Table, request: &v1::LlmServersRequest) -> Result<Planned, Diagnostic> {
    use v1::llm_servers_request::Op;

    let plain = |edits| Planned {
        edits,
        delete_key: None,
        forget: Vec::new(),
    };
    match &request.op {
        Some(Op::List(_)) => Ok(plain(Vec::new())),
        Some(Op::Add(add)) => servers::add(
            table,
            add.name.trim(),
            &add.endpoint,
            &add.models,
            add.activate,
        )
        .map(|edits| Planned {
            forget: vec![add.name.trim().to_owned()],
            ..plain(edits)
        }),
        Some(Op::Rename(rename)) => {
            servers::rename(table, rename.name.trim(), rename.new_name.trim()).map(|edits| {
                Planned {
                    forget: vec![
                        rename.name.trim().to_owned(),
                        rename.new_name.trim().to_owned(),
                    ],
                    ..plain(edits)
                }
            })
        }
        Some(Op::Remove(remove)) => {
            let replacement = remove.replacement.trim();
            let removal = servers::remove(
                table,
                remove.name.trim(),
                (!replacement.is_empty()).then_some(replacement),
            )?;
            Ok(Planned {
                edits: removal.edits,
                delete_key: removal.orphaned_key.filter(|_| remove.delete_key),
                forget: vec![remove.name.trim().to_owned()],
            })
        }
        Some(Op::Activate(activate)) => servers::activate(table, activate.name.trim()).map(plain),
        Some(Op::Edit(edit)) => {
            let edited = servers::edit(
                table,
                edit.name.trim(),
                edit.endpoint.as_deref(),
                edit.models.as_ref().map(|models| models.names.as_slice()),
            )?;
            // Eine neue Herkunft: Der Schlüssel der alten Adresse geht mit
            // (HUM-405, Review), und eine Bestätigung galt der alten.
            Ok(Planned {
                edits: edited.edits,
                delete_key: edited.orphaned_key,
                forget: if edited.origin_changed {
                    vec![edit.name.trim().to_owned()]
                } else {
                    Vec::new()
                },
            })
        }
        // Zuordnung und Bestätigung ändern `config.toml` nicht; sie prüfen
        // nur den Namen gegen die gespeicherten Server (HUM-405).
        Some(Op::Assign(assign)) => {
            let server = assign.server.trim();
            if !server.is_empty() {
                servers::require_saved(table, server)?;
            }
            target(assign)?;
            Ok(plain(Vec::new()))
        }
        Some(Op::ConfirmProject(confirm)) => {
            servers::require_saved(table, confirm.server.trim())?;
            Ok(plain(Vec::new()))
        }
        None => Err(Diagnostic::builder(CONFIG_021, Severity::Error)
            .why(
                "LlmServers changed nothing: the request names no operation (list, add, rename, \
                 remove, activate, edit, assign or confirm_project)"
                    .to_owned(),
            )
            .build()),
    }
}

/// Wem eine Zuordnung gilt (HUM-405).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target<'a> {
    /// Einem Profil der Sitzung, mit seinem Namen.
    Profile(&'a str),
    /// Einem Projektordner.
    Project(&'a std::path::Path),
}

/// Das Ziel einer Zuordnung.
///
/// # Errors
///
/// `CONFIG_021`, wenn die Zuordnung weder ein Profil noch einen
/// Projektordner nennt.
pub fn target(assign: &v1::llm_servers_request::Assign) -> Result<Target<'_>, Diagnostic> {
    use v1::llm_servers_request::assign::Target as Wire;
    match &assign.target {
        Some(Wire::Profile(name)) if !name.trim().is_empty() => Ok(Target::Profile(name.trim())),
        Some(Wire::ProjectDir(dir)) if !dir.trim().is_empty() => {
            Ok(Target::Project(std::path::Path::new(dir.trim())))
        }
        _ => Err(Diagnostic::builder(CONFIG_021, Severity::Error)
            .why(
                "the assignment names neither a session profile nor a project folder; nothing \
                 was assigned"
                    .to_owned(),
            )
            .fix(FixAction::CopyCommand(
                "humanitl llm server assign NAME --profile PROFILE".to_owned(),
            ))
            .build()),
    }
}

/// Wie ein Auftrag erfährt, ob gerade eine Sitzung die Sandbox hält.
///
/// Der Dienst gibt eine Funktion, die die übergebene Arbeit nur ausführt,
/// solange keine Sitzung läuft oder startet, und dabei jeden neuen Start
/// zurückhält (`SandboxService::while_idle`); sie sagt `false`, wenn sie
/// nicht ausgeführt hat. Ohne Sandbox und in Tests führt sie immer aus.
pub type IdleGate<'a> = &'a (dyn Fn(&mut dyn FnMut()) -> bool + Sync);

/// Führt immer aus: für einen Dienst ohne Sandbox.
pub fn always_idle(work: &mut dyn FnMut()) -> bool {
    work();
    true
}

/// Die beiden Schranken eines Auftrags (HUM-289, Review).
#[derive(Clone, Copy)]
pub struct Gates<'a> {
    /// Keine Sitzung läuft und kein Start hält seinen Anspruch: für das
    /// Entfernen des aktiven Servers.
    pub idle: IdleGate<'a>,
    /// Kein Start hält seinen Anspruch: für jedes Aktivieren und jedes
    /// Entfernen mit `delete_key`. Ein Start, der seine Konfiguration schon
    /// aufgelöst, den Schlüssel aber noch nicht geholt hat, fände sonst einen
    /// anderen Server aktiv oder seinen Schlüssel gelöscht. Eine laufende
    /// Sitzung hält ihren Schlüssel schon als Datei; für sie gilt ein
    /// Wechsel ab der nächsten.
    pub no_start: IdleGate<'a>,
}

/// Beide Schranken offen: für einen Dienst ohne Sandbox und für Tests.
pub const OPEN: Gates<'static> = Gates {
    idle: &always_idle,
    no_start: &always_idle,
};

/// Beantwortet einen Auftrag an `LlmServers`.
///
/// Blockiert: Es liest und schreibt `config.toml` und startet zum Löschen
/// eines Schlüssels das Werkzeug des Schlüsselbunds. Der Dienst ruft es in
/// [`tokio::task::spawn_blocking`].
///
/// Der ganze Ablauf steht unter [`servers::lock`]: das Lesen des Stands, das
/// Schreiben und das Löschen des Schlüssels, damit kein `LlmKey` dazwischen
/// einen Schlüssel an einen Eintrag hängt, der gerade geht. Den aktiven
/// Server entfernt er nur über `idle`, also nie, während eine Sitzung ihn
/// benutzt oder ein Start ihn gleich auflöst: Sitzungen benutzen nur den
/// aktiven Server.
///
/// # Errors
///
/// Wie [`plan`], dazu `CONFIG_001` für eine `config.toml`, die sich nicht
/// lesen lässt, `CONFIG_021`, wenn der aktive Server entfernt werden soll,
/// während eine Sitzung läuft, und die Befunde von
/// [`humanitl_config::edit::edit_with`], wenn die Datei nicht geschrieben
/// wurde.
pub fn answer(
    paths: &Paths,
    keys: &KeyStore,
    gates: &Gates<'_>,
    request: &v1::LlmServersRequest,
) -> Result<v1::LlmServerList, Diagnostic> {
    use v1::llm_servers_request::Op;

    // Entschieden wird unter der Sperre, am Stand, den der Auftrag ändert.
    let _order = servers::lock(paths)?;
    let (gate, why): (Option<IdleGate<'_>>, &str) = match &request.op {
        Some(Op::Remove(remove))
            if servers::active_name(&read_table(paths)?).as_deref() == Some(remove.name.trim()) =>
        {
            (
                Some(gates.idle),
                "the active server is in use by a running or starting session, and sessions use \
                 only the active server; remove it once the session has ended, or make another \
                 one active first",
            )
        }
        Some(Op::Remove(remove)) if remove.delete_key => (
            Some(gates.no_start),
            "a session is starting, and its key must not go away under it; try again once it \
             runs",
        ),
        Some(Op::Activate(_)) => (
            Some(gates.no_start),
            "a session is starting with the active server; switch once it runs, the switch then \
             applies to the next session",
        ),
        Some(Op::Edit(_)) => (
            Some(gates.no_start),
            "a session is starting and resolves its server now; change the server once it runs, \
             the change then applies to the next session",
        ),
        Some(Op::Assign(assign)) => return assign_and_list(paths, assign),
        Some(Op::ConfirmProject(confirm)) => return confirm_and_list(paths, confirm),
        _ => (None, ""),
    };
    let Some(gate) = gate else {
        return change(paths, keys, request);
    };
    let mut outcome = None;
    let ran = gate(&mut || outcome = Some(change(paths, keys, request)));
    match outcome {
        Some(outcome) if ran => outcome,
        _ => Err(Diagnostic::builder(CONFIG_021, Severity::Error)
            .why(format!("the model servers were not changed: {why}"))
            .fix(FixAction::CopyCommand("humanitl llm server ls".to_owned()))
            .build()),
    }
}

/// Schreibt einen Auftrag und löscht danach, wenn verlangt, den Schlüssel
/// eines entfernten Servers; unter [`servers::lock`].
fn change(
    paths: &Paths,
    keys: &KeyStore,
    request: &v1::LlmServersRequest,
) -> Result<v1::LlmServerList, Diagnostic> {
    let file = paths.config_path();
    // Bestätigungen fallen vor dem Schreiben weg, nicht danach: Scheitert
    // das, ändert sich nichts; scheitert danach das Schreiben, fragt der
    // nächste Start nur einmal mehr. Umgekehrt erbte ein umbenannter Server
    // eine Bestätigung, wenn das Löschen nach dem Schreiben scheiterte
    // (HUM-405, Review).
    let forget = plan(&read_table(paths)?, request)?.forget;
    if !forget.is_empty() {
        humanitl_config::assign::forget(paths, &forget)?;
    }
    let mut delete_key = None;
    humanitl_config::edit::edit_with(
        &file,
        |old| {
            let planned = plan(old, request)?;
            delete_key = planned.delete_key;
            Ok(planned.edits)
        },
        |candidate| crate::config_rpc::probe(paths, &file, candidate, servers::SERVERS),
    )?;
    let table = read_table(paths)?;
    let mut notes = Vec::new();
    // Noch einmal am geschriebenen Stand: Trägt inzwischen ein anderer
    // Eintrag denselben Verweis, bleibt der Schlüssel.
    if let Some(key_ref) =
        delete_key.filter(|key_ref| !servers::shared_by_other(&table, key_ref, ""))
        && let Err(mut left) = keys.clear(&key_ref)
    {
        // Der Server ist schon weg; der Schlüssel blieb. Das ist kein
        // Fehlschlag des Auftrags, aber der Mensch soll wissen, was liegt.
        left.severity = Severity::Warning;
        left.fix = keys.manual_clear(&key_ref);
        notes.push(convert::diagnostic_to_proto(&left));
    }
    Ok(listing(
        &servers::list(&table),
        &humanitl_config::profile_servers(paths),
        &uses_of(paths),
        notes,
    ))
}

/// Schreibt eine Zuordnung in die Profildatei oder das Projekt-Profil und
/// antwortet mit der Liste danach (HUM-405); unter [`servers::lock`].
///
/// Die Zuordnung eines Projekts zu einem Server über den Daemon ist die Wahl
/// des Menschen und gilt für diesen Ordner zugleich als bestätigt
/// ([`humanitl_config::assign::confirm`]).
fn assign_and_list(
    paths: &Paths,
    assign: &v1::llm_servers_request::Assign,
) -> Result<v1::LlmServerList, Diagnostic> {
    let table = read_table(paths)?;
    plan(
        &table,
        &v1::LlmServersRequest {
            op: Some(v1::llm_servers_request::Op::Assign(assign.clone())),
        },
    )?;
    let server = Some(assign.server.trim()).filter(|server| !server.is_empty());
    match target(assign)? {
        Target::Profile(name) => humanitl_config::assign::assign_profile(paths, name, server)?,
        Target::Project(dir) => {
            let dir = humanitl_config::assign::assign_project(dir, server)?;
            if let Some(server) = server {
                humanitl_config::assign::confirm(paths, &dir, server)?;
            }
        }
    }
    Ok(listing(
        &servers::list(&table),
        &humanitl_config::profile_servers(paths),
        &uses_of(paths),
        Vec::new(),
    ))
}

/// Merkt die Bestätigung der Wahl eines Projekts und antwortet mit der
/// Liste (HUM-405); unter [`servers::lock`].
fn confirm_and_list(
    paths: &Paths,
    confirm: &v1::llm_servers_request::ConfirmProject,
) -> Result<v1::LlmServerList, Diagnostic> {
    let table = read_table(paths)?;
    servers::require_saved(&table, confirm.server.trim())?;
    humanitl_config::assign::confirm(
        paths,
        std::path::Path::new(confirm.project_dir.trim()),
        confirm.server.trim(),
    )?;
    Ok(listing(
        &servers::list(&table),
        &humanitl_config::profile_servers(paths),
        &uses_of(paths),
        Vec::new(),
    ))
}

/// Wer welchen Server nutzt, für die Antwort (HUM-405).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Users {
    /// Jedes Profil der Sitzung außer `default`.
    pub profiles: Vec<humanitl_config::assign::ProfileUse>,
    /// Der Projektordner der Konfiguration, wenn es einen gibt.
    pub project: Option<humanitl_config::assign::ProjectUse>,
}

/// Wer welchen Server nutzt, von der Platte gelesen.
fn uses_of(paths: &Paths) -> Users {
    Users {
        profiles: humanitl_config::assign::profile_uses(paths),
        project: humanitl_config::assign::project_use(paths),
    }
}

/// Die Antwort aus einer Liste.
///
/// `uses` nennt je Server die Profile, die ihn mit `llm.server` wählen
/// (HUM-400, [`humanitl_config::profile_servers`]).
#[must_use]
pub fn listing(
    list: &ServerList,
    uses: &std::collections::BTreeMap<String, Vec<String>>,
    who: &Users,
    diagnostics: Vec<v1::Diagnostic>,
) -> v1::LlmServerList {
    v1::LlmServerList {
        servers: list
            .servers
            .iter()
            .map(|entry| v1::LlmServer {
                name: entry.name.clone(),
                endpoint: entry.endpoint.clone().unwrap_or_default(),
                models: entry.models.clone(),
                key_ref: entry.api_key_ref.clone().unwrap_or_default(),
                active: list.active.as_deref() == Some(entry.name.as_str()),
                profiles: uses.get(&entry.name).cloned().unwrap_or_default(),
            })
            .collect(),
        active: list.active.clone().unwrap_or_default(),
        implicit: list.implicit,
        diagnostics,
        project: who.project.as_ref().map(|project| v1::LlmProjectUse {
            dir: project.dir.display().to_string(),
            server: project.server.clone().unwrap_or_default(),
            confirmed: project.confirmed,
            effective_server: project.effective.clone().unwrap_or_default(),
            via_profile: project.via_profile.clone().unwrap_or_default(),
        }),
        profiles: who
            .profiles
            .iter()
            .map(|profile| v1::LlmProfileUse {
                name: profile.name.clone(),
                server: profile.server.clone().unwrap_or_default(),
                builtin: profile.builtin,
                description: profile.description.clone().unwrap_or_default(),
            })
            .collect(),
    }
}

/// `config.toml` als Tabelle; eine fehlende Datei ist leer.
///
/// # Errors
///
/// `CONFIG_001`, wenn die Datei da ist und sich nicht lesen oder nicht als
/// TOML lesen lässt.
pub fn read_table(paths: &Paths) -> Result<toml::Table, Diagnostic> {
    let file = paths.config_path();
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(toml::Table::new()),
        Err(error) => {
            return Err(Diagnostic::builder(CONFIG_001, Severity::Error)
                .why(format!("cannot read {}: {error}", file.display()))
                .build());
        }
    };
    text.trim_start_matches('\u{feff}')
        .parse()
        .map_err(|error: toml::de::Error| {
            Diagnostic::builder(CONFIG_001, Severity::Error)
                .why(format!("{} is not valid TOML: {error}", file.display()))
                .build()
        })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use humanitl_config::Env;

    use super::*;

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    fn op(op: v1::llm_servers_request::Op) -> v1::LlmServersRequest {
        v1::LlmServersRequest { op: Some(op) }
    }

    fn add(name: &str, endpoint: &str) -> v1::LlmServersRequest {
        op(v1::llm_servers_request::Op::Add(
            v1::llm_servers_request::Add {
                name: name.to_owned(),
                endpoint: endpoint.to_owned(),
                models: vec!["m".to_owned()],
                activate: false,
            },
        ))
    }

    fn write_config(paths: &Paths, text: &str) {
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(paths.config_path(), text).unwrap();
    }

    /// Anlegen, aktivieren, umbenennen, entfernen: jede Antwort ist die Liste
    /// danach, und die Datei trägt sie.
    #[test]
    fn the_operations_round_trip_through_config_toml() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        write_config(
            &paths,
            "# my config\n[llm]\nendpoint = \"http://home.lan:11434/\"\napi_key_ref = \"file:llm\"\n",
        );

        let listed = answer(
            &paths,
            &keys,
            &OPEN,
            &op(v1::llm_servers_request::Op::List(
                v1::llm_servers_request::List {},
            )),
        )
        .unwrap();
        assert!(listed.implicit);
        assert_eq!(listed.active, "default");

        let added = answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();
        assert!(!added.implicit);
        assert_eq!(added.servers.len(), 2);
        assert_eq!(added.active, "default");

        let activated = answer(
            &paths,
            &keys,
            &OPEN,
            &op(v1::llm_servers_request::Op::Activate(
                v1::llm_servers_request::Activate {
                    name: "cloud".to_owned(),
                },
            )),
        )
        .unwrap();
        assert_eq!(activated.active, "cloud");
        let text = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(text.starts_with("# my config"), "{text}");

        let resolved = humanitl_config::resolve(
            &humanitl_config::ProfileSelection::default(),
            None,
            paths.env(),
            &[],
        )
        .unwrap();
        assert_eq!(
            resolved.config.llm.endpoint.as_ref().map(url::Url::as_str),
            Some("https://api.example.com/v1")
        );
        assert_eq!(resolved.config.llm.api_key_ref, None);

        let renamed = answer(
            &paths,
            &keys,
            &OPEN,
            &op(v1::llm_servers_request::Op::Rename(
                v1::llm_servers_request::Rename {
                    name: "default".to_owned(),
                    new_name: "home".to_owned(),
                },
            )),
        )
        .unwrap();
        let home = renamed
            .servers
            .iter()
            .find(|server| server.name == "home")
            .unwrap();
        assert_eq!(home.key_ref, "file:llm");

        // Das Formular schreibt den flachen Schlüssel; der aktive Eintrag
        // bekommt denselben Wert, sonst setzte der nächste Start den alten
        // wieder ein.
        crate::config_rpc::set_expecting(
            &paths,
            "llm.endpoint",
            "https://api.example.com/v2",
            "cloud",
        )
        .expect("SetConfig writes");
        let table = read_table(&paths).unwrap();
        let list = servers::list(&table);
        let cloud = list
            .servers
            .iter()
            .find(|server| server.name == "cloud")
            .unwrap();
        assert_eq!(
            cloud.endpoint.as_deref(),
            Some("https://api.example.com/v2")
        );
    }

    /// Mit `delete_key` verschwindet der Schlüssel des entfernten Servers,
    /// ohne bleibt er; und der aktive geht nur mit Nachfolger.
    #[test]
    fn remove_deletes_the_key_only_when_asked() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let key = humanitl_config::secret::ApiKey::new("sk-0123456789abcdef").unwrap();
        keys.store_named("llm", &key).unwrap();
        write_config(
            &paths,
            "[llm]\nendpoint = \"http://home.lan:11434/\"\napi_key_ref = \"file:llm\"\n",
        );
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();

        let remove = |name: &str, replacement: &str, delete_key: bool| {
            op(v1::llm_servers_request::Op::Remove(
                v1::llm_servers_request::Remove {
                    name: name.to_owned(),
                    delete_key,
                    replacement: replacement.to_owned(),
                },
            ))
        };
        let refused = answer(&paths, &keys, &OPEN, &remove("default", "", true)).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert!(
            keys.file_path("llm").exists(),
            "a refused removal deleted the key"
        );

        let after = answer(&paths, &keys, &OPEN, &remove("default", "cloud", true)).unwrap();
        assert_eq!(after.active, "cloud");
        assert!(
            !keys.file_path("llm").exists(),
            "the key of the removed server stayed"
        );
    }

    /// Endpunkt und Modelle eines Servers ändern sich, ohne dass er Standard
    /// wird; eine Zuordnung oder Bestätigung mit einem Namen ohne Server
    /// ändert nichts und ist `CONFIG_021` (HUM-405).
    #[test]
    fn an_edit_keeps_the_default_and_unknown_names_are_refused() {
        use v1::llm_servers_request::{Assign, ConfirmProject, Edit, Op, assign, edit};

        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        write_config(&paths, "[llm]\nendpoint = \"http://home.lan:11434/\"\n");
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();

        let edited = answer(
            &paths,
            &keys,
            &OPEN,
            &op(Op::Edit(Edit {
                name: "cloud".to_owned(),
                endpoint: Some("https://api.example.org/v2".to_owned()),
                models: Some(edit::Models {
                    names: vec!["big".to_owned()],
                }),
            })),
        )
        .unwrap();
        assert_eq!(edited.active, "default");
        let cloud = edited
            .servers
            .iter()
            .find(|server| server.name == "cloud")
            .unwrap();
        assert_eq!(cloud.endpoint, "https://api.example.org/v2");
        assert_eq!(cloud.models, vec!["big".to_owned()]);
        assert!(!cloud.active);

        let before = std::fs::read_to_string(paths.config_path()).unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        for request in [
            Op::Assign(Assign {
                server: "gone".to_owned(),
                target: Some(assign::Target::ProjectDir(project.display().to_string())),
            }),
            Op::Assign(Assign {
                server: "cloud".to_owned(),
                target: None,
            }),
            Op::ConfirmProject(ConfirmProject {
                project_dir: project.display().to_string(),
                server: "gone".to_owned(),
            }),
        ] {
            let refused = answer(&paths, &keys, &OPEN, &op(request)).unwrap_err();
            assert_eq!(refused.code, CONFIG_021);
        }
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).unwrap(),
            before
        );
        assert!(!project.join(".humanitl").exists());
        assert!(!paths.project_servers_path().exists());
    }

    /// Bekommt ein Server eine Adresse mit anderer Herkunft, verliert er
    /// seinen Verweis, und sein Schlüssel wird gelöscht; derselbe Ursprung
    /// behält beides (HUM-405, Review).
    #[test]
    fn an_edit_to_another_origin_takes_the_key_away() {
        use v1::llm_servers_request::{Edit, Op};

        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let key = humanitl_config::secret::ApiKey::new("sk-0123456789abcdef").unwrap();
        keys.store_named("llm", &key).unwrap();
        write_config(
            &paths,
            "[llm]\nendpoint = \"http://home.lan:11434/\"\napi_key_ref = \"file:llm\"\n",
        );
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();
        let edit = |endpoint: &str| {
            op(Op::Edit(Edit {
                name: "default".to_owned(),
                endpoint: Some(endpoint.to_owned()),
                models: None,
            }))
        };
        let same = answer(&paths, &keys, &OPEN, &edit("http://home.lan:11434/v1")).unwrap();
        let home = same.servers.iter().find(|s| s.name == "default").unwrap();
        assert_eq!(home.key_ref, "file:llm");
        assert!(keys.file_path("llm").exists());

        let moved = answer(&paths, &keys, &OPEN, &edit("https://api.example.com/v1")).unwrap();
        let home = moved.servers.iter().find(|s| s.name == "default").unwrap();
        assert!(home.key_ref.is_empty(), "the key reference went along");
        assert!(
            !keys.file_path("llm").exists(),
            "the key of the old address was left behind"
        );
        let table = read_table(&paths).unwrap();
        assert!(
            servers::list(&table)
                .servers
                .iter()
                .all(|entry| entry.api_key_ref.is_none())
        );
    }

    /// Ein Auftrag ohne Operation ändert nichts.
    #[test]
    fn no_operation_is_config_021() {
        let table = toml::Table::new();
        let refused = plan(&table, &v1::LlmServersRequest::default()).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
    }

    /// Den aktiven Server entfernt der Dienst nicht, solange eine Sitzung die
    /// Sandbox hält; einen anderen schon (HUM-289, Review E).
    #[test]
    fn the_active_server_is_not_removed_while_a_session_holds_the_sandbox() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        write_config(
            &paths,
            "[llm]\nendpoint = \"http://home.lan:11434/\"\napi_key_ref = \"file:llm\"\n",
        );
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();
        let busy_fn = |_: &mut dyn FnMut()| false;
        let busy = Gates {
            idle: &busy_fn,
            no_start: &always_idle,
        };
        let remove = |name: &str, replacement: &str| {
            op(v1::llm_servers_request::Op::Remove(
                v1::llm_servers_request::Remove {
                    name: name.to_owned(),
                    delete_key: true,
                    replacement: replacement.to_owned(),
                },
            ))
        };
        let before = std::fs::read_to_string(paths.config_path()).unwrap();
        let refused = answer(&paths, &keys, &busy, &remove("default", "cloud")).unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).unwrap(),
            before
        );
        let other = answer(&paths, &keys, &busy, &remove("cloud", "")).unwrap();
        assert_eq!(other.servers.len(), 1);
    }

    /// Ein Wert, getippt für die Ansicht eines Servers, landet nach einem
    /// Wechsel nicht im anderen (HUM-289, Review).
    #[test]
    fn a_form_value_for_the_previous_server_is_refused_after_a_switch() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        write_config(&paths, "[llm]\nendpoint = \"http://home.lan:11434/\"\n");
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();
        crate::config_rpc::set_expecting(
            &paths,
            "llm.endpoint",
            "http://home.lan:8080/",
            "default",
        )
        .expect("the expected server is active");
        let switched = op(v1::llm_servers_request::Op::Activate(
            v1::llm_servers_request::Activate {
                name: "cloud".to_owned(),
            },
        ));
        answer(&paths, &keys, &OPEN, &switched).unwrap();
        let before = std::fs::read_to_string(paths.config_path()).unwrap();
        let refused = crate::config_rpc::set_expecting(
            &paths,
            "llm.endpoint",
            "http://home.lan:9090/",
            "default",
        )
        .unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).unwrap(),
            before
        );
    }

    /// Gibt es gespeicherte Server, schreibt `SetConfig` die Ansicht des
    /// aktiven nur mit dessen Namen (HUM-289, Review).
    #[test]
    fn a_form_value_without_a_server_name_is_refused_when_servers_exist() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        write_config(&paths, "[llm]\nendpoint = \"http://home.lan:11434/\"\n");
        crate::config_rpc::set_expecting(&paths, "llm.endpoint", "http://home.lan:8080/", "")
            .expect("without saved servers no name is needed");
        answer(
            &paths,
            &keys,
            &OPEN,
            &add("cloud", "https://api.example.com/v1"),
        )
        .unwrap();
        let before = std::fs::read_to_string(paths.config_path()).unwrap();
        let refused =
            crate::config_rpc::set_expecting(&paths, "llm.endpoint", "http://home.lan:9090/", "")
                .unwrap_err();
        assert_eq!(refused.code, CONFIG_021);
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).unwrap(),
            before
        );
        crate::config_rpc::set_expecting(&paths, "hold.timeout_secs", "42", "")
            .expect("other settings need no name");
    }
}
