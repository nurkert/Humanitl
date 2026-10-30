//! `LlmKey`: der Zugangsschlüssel des LLM-Endpunkts (HUM-140).
//!
//! Drei Operationen, und keine gibt den Schlüssel zurück:
//!
//! - `status` sagt, ob ein Schlüssel hinterlegt ist und wo
//!   (Schlüsselbund oder Datei), und meldet `LLM_009`, wenn
//!   `llm.api_key_ref` einen nennt, den der Speicher nicht hält.
//! - `set` legt ihn im Schlüsselbund ab, sonst in der Datei des Rückfalls
//!   (`CONFIG_019`), und richtet `llm.api_key_ref` in `config.toml` darauf.
//! - `remove` entfernt ihn und nimmt den Verweis aus `config.toml`.
//!
//! Mit `LlmKeyRequest.server` gilt jede Operation dem Schlüssel dieses
//! gespeicherten Servers (HUM-289): `set` legt ihn unter dem Namen des
//! Servers ab (`secret-service:humanitl/<name>`, unter macOS
//! `keychain:humanitl/<name>`, oder unter dem Namen, den sein Verweis schon
//! trägt) und richtet `llm.servers.<name>.api_key_ref` darauf, beim aktiven
//! Server auch `llm.api_key_ref`. Ohne Namen gilt der aktive Server, und ohne
//! Einträge `llm.api_key_ref` wie bisher.
//!
//! Die Reihenfolge von `set` ist Absicht: erst der Speicher, dann der Verweis.
//! Scheitert der Verweis, steht ein Schlüssel im Speicher, auf den nichts
//! zeigt — harmlos, und das nächste `set` überschreibt ihn. Andersherum stünde
//! ein Verweis in der Datei, dessen Schlüssel fehlt, und der nächste Start
//! bräche mit `LLM_009` ab.

use humanitl_config::secret::{KeyRef, KeyStore, StoreKind};
use humanitl_config::servers;
use humanitl_config::{Paths, ProfileSelection};
use humanitl_core::diagnostics::codes::{CONFIG_020, CONFIG_021};
use humanitl_core::{Diagnostic, Severity};
use zeroize::Zeroizing;

use crate::{convert, v1};

/// Der Pfad von `llm.api_key_ref` in `config.toml`.
const KEY_REF_PATH: [&str; 2] = ["llm", "api_key_ref"];

/// Beantwortet einen Auftrag an `LlmKey`.
///
/// Blockiert: Es startet das Werkzeug des Schlüsselbunds und schreibt
/// `config.toml`. Der Dienst ruft es in [`tokio::task::spawn_blocking`]. Der
/// ganze Ablauf steht unter [`servers::lock`], derselben Sperre wie
/// `LlmServers` (HUM-289).
///
/// # Errors
///
/// `CONFIG_020` ohne Operation, und wenn der Schlüssel keiner ist oder
/// sich nirgends ablegen oder entfernen lässt; `CONFIG_021` für einen
/// Server, den es nicht gibt, für ein `set` oder `remove` ohne aktiven
/// Server und für einen Server, der sich während des Ablegens geändert hat;
/// und die Befunde von [`humanitl_config::edit::edit_with`], wenn
/// `config.toml` nicht geschrieben wurde.
pub fn answer(
    paths: &Paths,
    keys: &KeyStore,
    request: v1::LlmKeyRequest,
) -> Result<v1::LlmKeyStatus, Diagnostic> {
    answer_with(paths, keys, request, &mut || {})
}

/// [`answer`] mit einem Haken zwischen Ablegen und Verweis, unter der Sperre;
/// für Tests, die zeigen, was in diesem Augenblick warten muss.
fn answer_with(
    paths: &Paths,
    keys: &KeyStore,
    request: v1::LlmKeyRequest,
    before_attach: &mut dyn FnMut(),
) -> Result<v1::LlmKeyStatus, Diagnostic> {
    let _order = servers::lock(paths)?;
    answer_locked(paths, keys, request, before_attach)
}

/// [`answer`] unter der Sperre; `before_attach` läuft beim Setzen für einen
/// Server zwischen dem Ablegen des Schlüssels und dem Verweis darauf. Tests
/// schreiben dort `config.toml` von außen um, wie es ein zweiter Prozess
/// täte.
fn answer_locked(
    paths: &Paths,
    keys: &KeyStore,
    request: v1::LlmKeyRequest,
    before_attach: &mut dyn FnMut(),
) -> Result<v1::LlmKeyStatus, Diagnostic> {
    let is_status = matches!(request.op, Some(v1::llm_key_request::Op::Status(_)));
    let named = !request.server.trim().is_empty();
    match target(paths, request.server.trim())? {
        // Ein Setzen oder Entfernen ohne Namen träfe den Server, der gerade
        // aktiv ist, und nicht unbedingt den, für den der Mensch den Schlüssel
        // eingegeben hat (HUM-289, Review): Gibt es gespeicherte Server,
        // nennt der Auftrag ihn, wie `SetConfig` mit `expected_server`.
        Target::Server(_) if !named && !is_status => {
            return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
                .why(
                    "LlmKey changed nothing: saved model servers exist, and the request names \
                     none; a key is set or removed only for the server it was entered for"
                        .to_owned(),
                )
                .fix(humanitl_core::FixAction::CopyCommand(
                    "humanitl llm key set --server NAME".to_owned(),
                ))
                .build());
        }
        Target::Server(server) => {
            return answer_for(paths, keys, &server, request.op, before_attach);
        }
        // Ohne gültigen aktiven Server gibt es keinen Schlüssel, nach dem
        // sich fragen ließe: der leere Stand, kein Fehler (Review).
        Target::NoActive if is_status => return Ok(v1::LlmKeyStatus::default()),
        Target::NoActive => {
            return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
                .why(
                    "LlmKey changed nothing: no model server is active, so it is unclear whose \
                     key is meant; name the server"
                        .to_owned(),
                )
                .fix(humanitl_core::FixAction::CopyCommand(
                    "humanitl llm key set --server NAME".to_owned(),
                ))
                .build());
        }
        Target::Flat => {}
    }
    match request.op {
        Some(v1::llm_key_request::Op::Status(_)) => Ok(status(paths, keys, Vec::new())),
        Some(v1::llm_key_request::Op::Set(mut set)) => {
            // Der Text der Nachricht wird beim Fallenlassen überschrieben.
            let text = Zeroizing::new(std::mem::take(&mut set.key));
            store(paths, keys, &text)
        }
        Some(v1::llm_key_request::Op::Remove(_)) => remove(paths, keys),
        None => Err(no_operation()),
    }
}

/// Wem ein Auftrag gilt.
enum Target {
    /// Den flachen Schlüsseln, ohne gespeicherte Server.
    Flat,
    /// Diesem gespeicherten Server.
    Server(String),
    /// Es gibt Server, aber keinen gültigen aktiven.
    NoActive,
}

/// Der Server, dem der Auftrag gilt.
///
/// Ein genannter Server gilt, wie er heißt. Ohne Namen gilt der aktive, wie
/// ihn die wirksame Konfiguration sieht (alle Ebenen, auch die Umgebung);
/// ohne Einträge bleibt es bei `llm.api_key_ref`, damit eine Datei von vor
/// HUM-289 sich nicht ändert, nur weil jemand einen Schlüssel setzt.
fn target(paths: &Paths, named: &str) -> Result<Target, Diagnostic> {
    if !named.is_empty() {
        servers::check_name(named)?;
        return Ok(Target::Server(named.to_owned()));
    }
    if let Ok(resolved) =
        humanitl_config::resolve(&ProfileSelection::default(), None, paths.env(), &[])
    {
        let llm = resolved.config.llm;
        if llm.servers.is_empty() {
            return Ok(Target::Flat);
        }
        return Ok(llm
            .active
            .filter(|name| llm.servers.contains_key(name))
            .map_or(Target::NoActive, Target::Server));
    }
    let table = crate::llm_servers::read_table(paths)?;
    if !servers::has_servers(&table) {
        return Ok(Target::Flat);
    }
    Ok(servers::active_name(&table).map_or(Target::NoActive, Target::Server))
}

/// Der Auftrag für den Schlüssel des Servers `server` (HUM-289).
fn answer_for(
    paths: &Paths,
    keys: &KeyStore,
    server: &str,
    op: Option<v1::llm_key_request::Op>,
    before_attach: &mut dyn FnMut(),
) -> Result<v1::LlmKeyStatus, Diagnostic> {
    let table = crate::llm_servers::read_table(paths)?;
    match op {
        Some(v1::llm_key_request::Op::Status(_)) => Ok(status_of(
            keys,
            servers::key_ref_of(&table, server)?,
            Vec::new(),
        )),
        Some(v1::llm_key_request::Op::Set(mut set)) => {
            let text = Zeroizing::new(std::mem::take(&mut set.key));
            let key = humanitl_config::secret::ApiKey::from_pasted(&text)?;
            let before = entry_of(&table, server)?;
            let name = servers::key_name_for(&table, server)?;
            let stored = keys.store_named(&name, &key)?;
            before_attach();
            attach(paths, keys, server, &before, &stored.key_ref)?;
            let notes = stored
                .note
                .iter()
                .map(convert::diagnostic_to_proto)
                .collect();
            Ok(status_of(keys, Some(stored.key_ref), notes))
        }
        Some(v1::llm_key_request::Op::Remove(_)) => {
            // Ein Verweis, den ein anderer Server auch trägt, löscht nicht
            // dessen Schlüssel mit, wie beim Entfernen eines Servers.
            if let Some(key_ref) = servers::key_ref_of(&table, server)?
                && !servers::shared_by_other(&table, &key_ref, server)
            {
                keys.clear(&key_ref)?;
            }
            write_server_ref(paths, server, None)?;
            Ok(status_of(keys, None, Vec::new()))
        }
        None => Err(no_operation()),
    }
}

/// Der Eintrag `server`, wie er jetzt dasteht.
fn entry_of(table: &toml::Table, server: &str) -> Result<servers::ServerEntry, Diagnostic> {
    servers::list(table)
        .servers
        .into_iter()
        .find(|entry| entry.name == server)
        .ok_or_else(|| {
            // Dieselbe Absage wie `key_ref_of` für einen unbekannten Namen.
            servers::key_ref_of(table, server).err().unwrap_or_else(|| {
                Diagnostic::builder(CONFIG_021, Severity::Error)
                    .why(format!("there is no server named {server}"))
                    .build()
            })
        })
}

/// Hängt den eben abgelegten Schlüssel an den Server, wenn er noch derselbe
/// ist (HUM-289, Review D).
///
/// Unter der Sperre von `config.toml` wird nachgesehen, ob der Eintrag noch
/// da ist, denselben Endpunkt hat und keinen anderen Verweis trägt als den,
/// unter dem der Schlüssel eben abgelegt wurde. Hat ihn jemand dazwischen
/// entfernt, neu angelegt oder auf eine andere Adresse gerichtet, wird der
/// Schlüssel nicht angehängt: Er gehört zu dem Server, für den ihn der Mensch
/// eingegeben hat. Der abgelegte Eintrag im Speicher wird dann gelöscht, und
/// ein Verweis darauf, der noch irgendwo steht, verschwindet mit ihm; lieber
/// fehlt ein Schlüssel, als dass er an eine fremde Herkunft geht.
fn attach(
    paths: &Paths,
    keys: &KeyStore,
    server: &str,
    before: &servers::ServerEntry,
    key_ref: &KeyRef,
) -> Result<(), Diagnostic> {
    let file = paths.config_path();
    let attached = humanitl_config::edit::edit_with(
        &file,
        |old| {
            let now = servers::list(old)
                .servers
                .into_iter()
                .find(|entry| entry.name == server);
            let same = now.as_ref().is_some_and(|now| {
                now.endpoint == before.endpoint
                    && now
                        .api_key_ref
                        .as_deref()
                        .and_then(KeyRef::parse)
                        .is_none_or(|own| own.name() == key_ref.name())
            });
            if !same {
                return Err(Diagnostic::builder(CONFIG_021, Severity::Error)
                    .why(format!(
                        "the access key was not attached: the server {server} changed while the \
                         key was being stored (it was removed, added again or got another \
                         endpoint); set the key again for the server as it is now"
                    ))
                    .fix(humanitl_core::FixAction::CopyCommand(format!(
                        "humanitl llm key set --server {server}"
                    )))
                    .build());
            }
            servers::set_key_ref(old, server, Some(key_ref))
        },
        |_| Ok(()),
    );
    let Err(refused) = attached else {
        return Ok(());
    };
    // Der Schlüssel liegt schon im Speicher; er darf an keinem Eintrag
    // hängen, für den er nicht eingegeben wurde.
    let _detached = humanitl_config::edit::edit_with(
        &file,
        |old| {
            let mut edits = Vec::new();
            for entry in servers::list(old).servers {
                if entry
                    .api_key_ref
                    .as_deref()
                    .and_then(KeyRef::parse)
                    .is_some_and(|own| own.name() == key_ref.name())
                {
                    edits.extend(servers::set_key_ref(old, &entry.name, None)?);
                }
            }
            Ok(edits)
        },
        |_| Ok(()),
    );
    let _cleared = keys.clear(key_ref);
    Err(refused)
}

/// Richtet den Verweis des Servers `server` auf den Schlüssel, oder nimmt
/// ihn heraus; beim aktiven Server auch `llm.api_key_ref`.
fn write_server_ref(
    paths: &Paths,
    server: &str,
    key_ref: Option<&KeyRef>,
) -> Result<(), Diagnostic> {
    humanitl_config::edit::edit_with(
        &paths.config_path(),
        |old| servers::set_key_ref(old, server, key_ref),
        |_| Ok(()),
    )?;
    Ok(())
}

/// `CONFIG_020` für einen Auftrag ohne Operation.
fn no_operation() -> Diagnostic {
    Diagnostic::builder(CONFIG_020, Severity::Error)
        .why(
            "LlmKey changed nothing: the request names no operation (status, set or remove)"
                .to_owned(),
        )
        .build()
}

/// Legt den Schlüssel ab und richtet den Verweis darauf.
fn store(paths: &Paths, keys: &KeyStore, text: &str) -> Result<v1::LlmKeyStatus, Diagnostic> {
    let key = humanitl_config::secret::ApiKey::from_pasted(text)?;
    let stored = keys.store(&key)?;
    write_ref(paths, Some(&stored.key_ref))?;
    let notes = stored
        .note
        .iter()
        .map(convert::diagnostic_to_proto)
        .collect();
    Ok(status(paths, keys, notes))
}

/// Entfernt den Schlüssel und den Verweis.
fn remove(paths: &Paths, keys: &KeyStore) -> Result<v1::LlmKeyStatus, Diagnostic> {
    if let Some(key_ref) = configured_ref(paths) {
        keys.clear(&key_ref)?;
    }
    write_ref(paths, None)?;
    Ok(status(paths, keys, Vec::new()))
}

/// Schreibt `llm.api_key_ref`, oder nimmt ihn heraus.
fn write_ref(paths: &Paths, key_ref: Option<&KeyRef>) -> Result<(), Diagnostic> {
    let segments: Vec<String> = KEY_REF_PATH.iter().map(|part| (*part).to_owned()).collect();
    let value = key_ref.map(|key_ref| toml::Value::String(key_ref.to_string()));
    humanitl_config::edit::set_value(&paths.config_path(), &segments, value.as_ref(), |_| Ok(()))?;
    Ok(())
}

/// Der Verweis, wie ihn die Konfiguration ohne Profilwunsch trägt.
///
/// Gelesen wird frisch aus der Datei und nicht aus der Auflösung des
/// Daemon-Starts: Ein `set` von eben gilt für die Frage „ist einer da?" sofort.
fn configured_ref(paths: &Paths) -> Option<KeyRef> {
    let resolved =
        humanitl_config::resolve(&ProfileSelection::default(), None, paths.env(), &[]).ok()?;
    KeyRef::parse(resolved.config.llm.api_key_ref.as_deref()?)
}

/// Der Stand: ob ein Schlüssel da ist, wo, und was dazu zu sagen ist.
///
/// Ob er da ist, sagt der Speicher selbst und nicht der Verweis: Ein Verweis
/// auf einen gelöschten Eintrag ist kein „gesetzt", sondern `LLM_009`. Der
/// Schlüssel, den die Nachfrage dafür liest, verlässt diese Funktion nicht.
#[must_use]
pub fn status(
    paths: &Paths,
    keys: &KeyStore,
    mut diagnostics: Vec<v1::Diagnostic>,
) -> v1::LlmKeyStatus {
    status_of(
        keys,
        configured_ref(paths),
        std::mem::take(&mut diagnostics),
    )
}

/// Der Stand zu einem Verweis, wie [`status`] ihn beschreibt.
fn status_of(
    keys: &KeyStore,
    key_ref: Option<KeyRef>,
    mut diagnostics: Vec<v1::Diagnostic>,
) -> v1::LlmKeyStatus {
    let Some(key_ref) = key_ref else {
        return v1::LlmKeyStatus {
            diagnostics,
            ..v1::LlmKeyStatus::default()
        };
    };
    let set = match keys.lookup(&key_ref) {
        Ok(_key) => true,
        Err(diagnostic) => {
            diagnostics.push(convert::diagnostic_to_proto(&diagnostic));
            false
        }
    };
    v1::LlmKeyStatus {
        set,
        store: store_to_proto(key_ref.store()) as i32,
        key_ref: key_ref.to_string(),
        diagnostics,
    }
}

/// Der Ort in seiner Wire-Form.
const fn store_to_proto(store: StoreKind) -> v1::LlmKeyStore {
    match store {
        // Der Wert meint auf dem Draht den Schlüsselbund des Systems; welcher,
        // sagt der Verweis (HUM-349).
        StoreKind::SecretService | StoreKind::Keychain => v1::LlmKeyStore::SecretService,
        StoreKind::File => v1::LlmKeyStore::File,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use humanitl_config::Env;

    use super::*;

    const SECRET: &str = "sk-live-key-0123456789";

    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    /// Eine Attrappe von `secret-tool` mit einem Ablageort im Testverzeichnis.
    fn fake_tool(dir: &Path) -> PathBuf {
        let store = dir.join("keyring");
        std::fs::create_dir_all(&store).unwrap();
        let script = dir.join("secret-tool");
        crate::test_support::write_executable(
            &script,
            &format!(
                "#!/bin/sh\nfor last; do :; done\ncase \"$1\" in\n  store) cat > '{0}'/\"$last\" \
                 ;;\n  lookup) [ -f '{0}'/\"$last\" ] || exit 1; cat '{0}'/\"$last\" ;;\n  \
                 clear) rm -f '{0}'/\"$last\" ;;\nesac\n",
                store.display()
            ),
        );
        script
    }

    fn request(op: v1::llm_key_request::Op) -> v1::LlmKeyRequest {
        v1::LlmKeyRequest {
            op: Some(op),
            ..v1::LlmKeyRequest::default()
        }
    }

    fn set_op(key: &str) -> v1::LlmKeyRequest {
        request(v1::llm_key_request::Op::Set(v1::llm_key_request::Set {
            key: key.to_owned(),
        }))
    }

    /// Gesetzt heißt: im Schlüsselbund, der Verweis in `config.toml`, und
    /// weder die Datei noch die Antwort trägt den Schlüssel (HUM-140).
    #[test]
    fn set_stores_the_key_and_only_the_reference_reaches_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));

        let answer = answer(&paths, &keys, set_op(SECRET)).unwrap();
        assert!(answer.set);
        assert_eq!(answer.store, v1::LlmKeyStore::SecretService as i32);
        assert_eq!(answer.key_ref, "secret-service:humanitl/llm");
        assert!(!format!("{answer:?}").contains(SECRET), "{answer:?}");

        let config = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(
            config.contains("api_key_ref = \"secret-service:humanitl/llm\""),
            "{config}"
        );
        assert!(!config.contains(SECRET), "{config}");

        let status = super::answer(
            &paths,
            &keys,
            request(v1::llm_key_request::Op::Status(
                v1::llm_key_request::Status {},
            )),
        )
        .unwrap();
        assert!(status.set);
    }

    /// Unter macOS trägt der Verweis den Ort, und der Wert auf dem Draht
    /// bleibt „Schlüsselbund des Systems" (HUM-349).
    #[test]
    fn a_key_in_the_keychain_answers_with_its_reference() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let store = dir.path().join("keychain");
        std::fs::create_dir_all(&store).unwrap();
        let script = dir.path().join("security");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\nif [ \"$1\" = -i ]; then sed -e 's/.* -w \"//' -e 's/\"$//' \
                 > '{0}/llm'; exit 0; fi\n[ -f '{0}/llm' ] || exit 44\n\
                 [ \"$1\" = find-generic-password ] && exec cat '{0}/llm'\n\
                 rm '{0}/llm'\n",
                store.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let keys = KeyStore::for_paths(&paths).with_keychain_tool(Some(script));

        let stored = answer(&paths, &keys, set_op(SECRET)).unwrap();
        assert!(stored.set, "{stored:?}");
        assert!(stored.diagnostics.is_empty(), "{stored:?}");
        assert_eq!(stored.store, v1::LlmKeyStore::SecretService as i32);
        assert_eq!(stored.key_ref, "keychain:humanitl/llm");
        let config = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(
            config.contains("api_key_ref = \"keychain:humanitl/llm\""),
            "{config}"
        );
        assert!(!config.contains(SECRET), "{config}");
    }

    /// Ohne Schlüsselbund liegt er in der Datei, und die Antwort sagt das.
    #[test]
    fn without_a_keyring_the_answer_names_the_file_and_config_019() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let answer = answer(&paths, &keys, set_op(SECRET)).unwrap();
        assert!(answer.set);
        assert_eq!(answer.store, v1::LlmKeyStore::File as i32);
        assert_eq!(answer.key_ref, "file:llm");
        assert_eq!(answer.diagnostics[0].code, "CONFIG_019");
    }

    /// Ein Verweis ohne Schlüssel ist nicht „gesetzt", sondern `LLM_009` mit
    /// Vorschlag.
    #[test]
    fn a_reference_without_its_key_is_llm_009() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(paths.config_path(), "[llm]\napi_key_ref = \"file:llm\"\n").unwrap();
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let status = status(&paths, &keys, Vec::new());
        assert!(!status.set);
        assert_eq!(status.diagnostics[0].code, "LLM_009");
        assert!(status.diagnostics[0].fix.is_some());
    }

    /// `remove` nimmt Schlüssel und Verweis weg.
    #[test]
    fn remove_takes_key_and_reference() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        answer(&paths, &keys, set_op(SECRET)).unwrap();
        let removed = answer(
            &paths,
            &keys,
            request(v1::llm_key_request::Op::Remove(
                v1::llm_key_request::Remove {},
            )),
        )
        .unwrap();
        assert!(!removed.set);
        assert!(removed.key_ref.is_empty());
        let config = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(!config.contains("api_key_ref"), "{config}");
        assert!(!dir.path().join("keyring/llm").exists());
    }

    /// Ein Schlüssel, eingefügt samt Anführungszeichen und Leerraum, landet
    /// ohne beides im Speicher und wirkt danach (HUM-283).
    #[test]
    fn a_key_pasted_with_quotes_is_stored_without_them() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        let answer = answer(&paths, &keys, set_op(&format!(" \"{SECRET}\"\n"))).unwrap();
        assert!(answer.set);
        let key_ref = KeyRef::parse(&answer.key_ref).unwrap();
        let stored = keys.lookup(&key_ref).unwrap();
        assert!(
            stored.expose() == SECRET,
            "the stored key still carries the quotes or blanks"
        );
    }

    /// Leerraum im Inneren ist kein Rest der Zwischenablage: abgelehnt, nichts
    /// geschrieben, und der Befund wiederholt den Schlüssel nicht (HUM-283).
    #[test]
    fn a_key_with_inner_white_space_is_refused_without_echoing_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let refused = answer(&paths, &keys, set_op("\"sk-live-key 0123456789\"")).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_020");
        assert!(
            !refused.why.contains("sk-live-key") && !refused.why.contains("0123456789"),
            "the refusal repeats the key"
        );
        assert!(!paths.config_path().exists());
    }

    /// Ein Schlüssel mit Zeilenumbruch wird nicht abgelegt.
    #[test]
    fn a_key_with_a_line_break_is_config_020_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        let refused = answer(&paths, &keys, set_op("sk-1\n")).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_020");
        assert!(!paths.config_path().exists());
    }

    fn for_server(mut request: v1::LlmKeyRequest, server: &str) -> v1::LlmKeyRequest {
        request.server = server.to_owned();
        request
    }

    /// Der Schlüssel eines nicht aktiven Servers landet unter dessen Namen,
    /// sein Verweis im Eintrag, und der flache Verweis des aktiven bleibt
    /// (HUM-289). Der übernommene Eintrag `default` behält `humanitl/llm`.
    #[test]
    fn a_server_key_goes_under_its_own_name_and_leaves_the_active_one() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        answer(&paths, &keys, set_op(SECRET)).unwrap();
        crate::llm_servers::answer(
            &paths,
            &keys,
            &crate::llm_servers::OPEN,
            &v1::LlmServersRequest {
                op: Some(v1::llm_servers_request::Op::Add(
                    v1::llm_servers_request::Add {
                        name: "cloud".to_owned(),
                        endpoint: "https://api.example.com/v1".to_owned(),
                        models: Vec::new(),
                        activate: false,
                    },
                )),
            },
        )
        .unwrap();

        let other = "sk-other-key-9876543210";
        let cloud = answer(&paths, &keys, for_server(set_op(other), "cloud")).unwrap();
        assert_eq!(cloud.key_ref, "secret-service:humanitl/cloud");
        assert!(cloud.set);
        assert!(!format!("{cloud:?}").contains(other));

        let config = std::fs::read_to_string(paths.config_path()).unwrap();
        let table: toml::Table = config.parse().unwrap();
        let llm = table["llm"].as_table().unwrap();
        assert_eq!(
            llm["api_key_ref"].as_str(),
            Some("secret-service:humanitl/llm"),
            "the active server lost its key: {config}"
        );
        assert_eq!(
            llm["servers"]["default"]["api_key_ref"].as_str(),
            Some("secret-service:humanitl/llm")
        );
        assert_eq!(
            llm["servers"]["cloud"]["api_key_ref"].as_str(),
            Some("secret-service:humanitl/cloud")
        );
        assert!(!config.contains(other) && !config.contains(SECRET));
        let stored = keys
            .lookup(&KeyRef::parse("secret-service:humanitl/llm").unwrap())
            .unwrap();
        assert!(stored.expose() == SECRET, "the active key was overwritten");

        let status = answer(
            &paths,
            &keys,
            for_server(
                request(v1::llm_key_request::Op::Status(
                    v1::llm_key_request::Status {},
                )),
                "cloud",
            ),
        )
        .unwrap();
        assert!(status.set);

        let unknown = answer(
            &paths,
            &keys,
            for_server(
                request(v1::llm_key_request::Op::Status(
                    v1::llm_key_request::Status {},
                )),
                "nope",
            ),
        )
        .unwrap_err();
        assert_eq!(unknown.code.as_str(), "CONFIG_021");
    }

    /// Ein Server, der zwischen dem Ablegen des Schlüssels und dem Verweis
    /// darauf entfernt und mit anderem Endpunkt neu angelegt wird, bekommt
    /// den Schlüssel nicht, und der abgelegte Eintrag verschwindet (HUM-289,
    /// Review D).
    #[test]
    fn a_server_replaced_while_its_key_is_stored_does_not_get_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            "[llm]\nactive = \"home\"\nendpoint = \"http://home.lan/\"\n\
             [llm.servers.home]\nendpoint = \"http://home.lan/\"\n\
             [llm.servers.cloud]\nendpoint = \"https://api.example.com/v1\"\n",
        )
        .unwrap();
        let config = paths.config_path();
        let mut replace = || {
            std::fs::write(
                &config,
                "[llm]\nactive = \"home\"\nendpoint = \"http://home.lan/\"\n\
                 [llm.servers.home]\nendpoint = \"http://home.lan/\"\n\
                 [llm.servers.cloud]\nendpoint = \"https://evil.example.net/v1\"\n",
            )
            .unwrap();
        };
        let refused = answer_locked(
            &paths,
            &keys,
            for_server(set_op("sk-cloud-0123456789abcdef"), "cloud"),
            &mut replace,
        )
        .unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_021");
        let text = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(
            !text.contains("humanitl/cloud"),
            "the key was attached to the replaced server: {text}"
        );
        assert!(
            !dir.path().join("keyring/cloud").exists(),
            "the stored key stayed in the keyring"
        );
    }

    /// Status ohne gültigen aktiven Server ist der leere Stand, kein Fehler.
    #[test]
    fn status_without_an_active_server_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(None);
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            "[llm.servers.a]\nendpoint = \"http://a.lan/\"\napi_key_ref = \"file:a\"\n",
        )
        .unwrap();
        let status = answer(
            &paths,
            &keys,
            request(v1::llm_key_request::Op::Status(
                v1::llm_key_request::Status {},
            )),
        )
        .unwrap();
        assert!(!status.set);
        assert!(status.key_ref.is_empty());
        let refused = answer(&paths, &keys, set_op(SECRET)).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_021");
    }

    /// Während `LlmKey` einen Schlüssel für einen Server ablegt, wartet ein
    /// `SetConfig` auf dessen Endpunkt, bis der Verweis steht: Endpunkt und
    /// Verweis ändern sich nie mitten im Ablegen (HUM-289, Review).
    #[test]
    fn a_form_write_waits_while_a_key_is_stored() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            "[llm]\nactive = \"home\"\nendpoint = \"http://home.lan/\"\n\
             [llm.servers.home]\nendpoint = \"http://home.lan/\"\n",
        )
        .unwrap();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let mut writer = None;
        let mut blocked = None;
        let mut hook = || {
            let paths = paths.clone();
            let done_tx = done_tx.clone();
            writer = Some(std::thread::spawn(move || {
                let written = crate::config_rpc::set_expecting(
                    &paths,
                    "llm.endpoint",
                    "http://other.lan/",
                    "home",
                );
                done_tx.send(written.is_ok()).unwrap();
            }));
            blocked = Some(
                done_rx.recv_timeout(std::time::Duration::from_secs(1))
                    == Err(std::sync::mpsc::RecvTimeoutError::Timeout),
            );
        };
        let stored = answer_with(
            &paths,
            &keys,
            for_server(set_op("sk-home-0123456789abcdef"), "home"),
            &mut hook,
        )
        .expect("the key is attached to the unchanged server");
        assert_eq!(
            blocked,
            Some(true),
            "SetConfig wrote while the key was being stored"
        );
        writer.unwrap().join().unwrap();
        assert!(
            done_rx.recv().unwrap(),
            "SetConfig writes once the key is attached"
        );
        assert_eq!(stored.key_ref, "secret-service:humanitl/home");
        let text = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(text.contains("http://other.lan/"), "{text}");
    }

    /// Gibt es gespeicherte Server, setzt und entfernt `LlmKey` nur mit
    /// Namen; der Stand ohne Namen bleibt der des aktiven (HUM-289, Review).
    #[test]
    fn an_unnamed_set_or_remove_is_refused_when_servers_exist() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let keys = KeyStore::for_paths(&paths).with_secret_tool(Some(fake_tool(dir.path())));
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            "[llm]\nactive = \"home\"\nendpoint = \"http://home.lan/\"\n\
             [llm.servers.home]\nendpoint = \"http://home.lan/\"\n",
        )
        .unwrap();
        let before = std::fs::read_to_string(paths.config_path()).unwrap();
        let refused = answer(&paths, &keys, set_op(SECRET)).unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_021");
        let refused = answer(
            &paths,
            &keys,
            request(v1::llm_key_request::Op::Remove(
                v1::llm_key_request::Remove {},
            )),
        )
        .unwrap_err();
        assert_eq!(refused.code.as_str(), "CONFIG_021");
        assert_eq!(
            std::fs::read_to_string(paths.config_path()).unwrap(),
            before
        );
        assert!(!dir.path().join("keyring/home").exists());
        answer(
            &paths,
            &keys,
            request(v1::llm_key_request::Op::Status(
                v1::llm_key_request::Status {},
            )),
        )
        .expect("the state needs no name");
        answer(&paths, &keys, for_server(set_op(SECRET), "home")).expect("named, it is set");
    }
}
