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
//! Die Reihenfolge von `set` ist Absicht: erst der Speicher, dann der Verweis.
//! Scheitert der Verweis, steht ein Schlüssel im Speicher, auf den nichts
//! zeigt — harmlos, und das nächste `set` überschreibt ihn. Andersherum stünde
//! ein Verweis in der Datei, dessen Schlüssel fehlt, und der nächste Start
//! bräche mit `LLM_009` ab.

use humanitl_config::secret::{KeyRef, KeyStore, StoreKind};
use humanitl_config::{Paths, ProfileSelection};
use humanitl_core::Diagnostic;
use humanitl_core::diagnostics::codes::CONFIG_020;
use zeroize::Zeroizing;

use crate::{convert, v1};

/// Der Pfad von `llm.api_key_ref` in `config.toml`.
const KEY_REF_PATH: [&str; 2] = ["llm", "api_key_ref"];

/// Beantwortet einen Auftrag an `LlmKey`.
///
/// Blockiert: Es startet `secret-tool` und schreibt `config.toml`. Der Dienst
/// ruft es in [`tokio::task::spawn_blocking`].
///
/// # Errors
///
/// `CONFIG_020` ohne Operation, und wenn der Schlüssel keiner ist oder
/// sich nirgends ablegen oder entfernen lässt, und die Befunde von
/// [`humanitl_config::edit::set_value`], wenn `config.toml` nicht geschrieben
/// wurde.
pub fn answer(
    paths: &Paths,
    keys: &KeyStore,
    request: v1::LlmKeyRequest,
) -> Result<v1::LlmKeyStatus, Diagnostic> {
    match request.op {
        Some(v1::llm_key_request::Op::Status(_)) => Ok(status(paths, keys, Vec::new())),
        Some(v1::llm_key_request::Op::Set(mut set)) => {
            // Der Text der Nachricht wird beim Fallenlassen überschrieben.
            let text = Zeroizing::new(std::mem::take(&mut set.key));
            store(paths, keys, &text)
        }
        Some(v1::llm_key_request::Op::Remove(_)) => remove(paths, keys),
        None => Err(Diagnostic::builder(CONFIG_020, humanitl_core::Severity::Error)
            .why("LlmKey changed nothing: the request names no operation (status, set or remove)".to_owned())
            .build()),
    }
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
    let Some(key_ref) = configured_ref(paths) else {
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
        StoreKind::SecretService => v1::LlmKeyStore::SecretService,
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
        v1::LlmKeyRequest { op: Some(op) }
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
}
