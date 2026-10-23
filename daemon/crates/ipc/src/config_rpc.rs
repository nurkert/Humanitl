//! `GetConfig` und `SetConfig`: die Konfiguration lesen und die Einstellungen
//! des Formulars schreiben (HUM-151, geweitet in HUM-069).
//!
//! `SetConfig` nimmt zwei Arten Auftrag an, und sonst keinen:
//!
//! - **Die Einstellungen des Formulars** ([`FORM_KEYS`]): die wenigen Werte,
//!   die ein Mensch im MVP in der Oberfläche ändert (zweite
//!   Umfangsentscheidung vom 2026-09-18 in `backlog/sprint-4.md`). Der Wert
//!   kommt als Text, so wie `humanitl config set KEY VALUE` ihn bekommt, und
//!   wird mit derselben Lesart gelesen und geprüft
//!   ([`humanitl_config::value`]). Alles andere bleibt in `config.toml`.
//! - **Eine CA-Variable unter `sandbox.env`**, deren Wert das Zertifikat ist,
//!   das Humanitl in der Sandbox einhängt — der Vorschlag eines `TLS_001`
//!   (HUM-151). Ein Client ändert damit keine Einhängung, keinen Befehl und
//!   kein Ziel des Verkehrs; warum das nicht in seine Hand gehört, steht an
//!   [`crate::session::check_override_key`].
//!
//! Geschrieben wird in beiden Fällen über [`humanitl_config::edit::set_value`],
//! den einen Weg in `config.toml`: nur der eine Wert ändert sich, Kommentare und
//! Reihenfolge bleiben, nie eine halbe Datei. Vor dem Ersetzen lädt die
//! Gegenprobe die Nebendatei mit allen übrigen Ebenen (`probe`).
//!
//! The running daemon uses a written value without a restart (HUM-277):
//! every sandbox start resolves its configuration afresh (passthrough rule,
//! key mask, hold mode), `Sandbox(Plan)` refreshes while no session holds the
//! sandbox, and `Doctor` reads the file again. A running session keeps the
//! configuration it started with. Only what the daemon builds once at its own
//! start (the proxy's TLS stack, hence `tls.trust_system_store`, limits,
//! recording) waits for its next start; the settings form says so. General
//! live reload stays deferred (BACKLOG.md section 9, item 17).

use std::path::{Path, PathBuf};

use humanitl_config::value::{check_value, parse_value, to_toml};
use humanitl_config::{Paths, ProfileSelection, Resolved, schema};
use humanitl_core::diagnostics::codes::CONFIG_014;
use humanitl_core::{Diagnostic, Severity};
use humanitl_sandbox::profile::CA_CERT_DST;

use crate::{convert, v1};

/// Der Anfang jedes Schlüssels einer Variable der Sandbox.
pub const SANDBOX_ENV_PREFIX: &str = "sandbox.env.";

/// Die Schlüssel, die das Formular der Oberfläche schreibt (HUM-069).
///
/// Der Zugangsschlüssel steht nicht darin: Er geht nie durch `SetConfig` und
/// nie in `config.toml`, sondern über `LlmKey` in den Schlüsselbund
/// ([`crate::llm_key`]); in der Datei steht nur sein Verweis
/// `llm.api_key_ref`, und den schreibt `LlmKey` selbst.
pub const FORM_KEYS: &[&str] = &[
    "hold.timeout_secs",
    "hold.ask_mode",
    "ui.language",
    "ui.theme",
    "ui.notifications",
    "sandbox.work_dir",
    "llm.endpoint",
    "llm.models",
    // Das Opt-out des Vertrauens in die Zertifikatsablage des Systems
    // (HUM-257). Im Projekt-Profil gesperrt, hier schreibt nur der Mensch.
    "tls.trust_system_store",
];

/// So viele Zeichen eines abgelehnten Schlüssels stehen im Befund. Der Text
/// reist im Status mit, und ein Client bestimmt die Länge des Schlüssels.
const SHOWN_KEY_CHARS: usize = 80;

/// Setzt den Wert und antwortet mit der Konfiguration, wie sie danach gilt.
///
/// Blockiert, weil es eine Datei liest und schreibt; der Dienst ruft es in
/// [`tokio::task::spawn_blocking`].
///
/// # Errors
///
/// `CONFIG_014` für einen Schlüssel, den `SetConfig` nicht schreibt,
/// `CONFIG_003` für einen Wert, den das Schema oder die Prüfung ablehnt, und
/// die Befunde von [`humanitl_config::edit::set_value`], wenn die Datei nicht
/// geändert wurde. Geschrieben ist in keinem der Fälle etwas.
pub fn set(paths: &Paths, key: &str, value: &str) -> Result<v1::ConfigSnapshot, Diagnostic> {
    set_expecting(paths, key, value, "")
}

/// Wie [`set`]; für `llm.endpoint` und `llm.models` mit dem gespeicherten
/// Server, dessen Ansicht der Client bearbeitet hat (HUM-289,
/// `SetConfigRequest.expected_server`). Ist ein anderer aktiv, wird nichts
/// geschrieben.
///
/// # Errors
///
/// Wie [`set`], dazu `CONFIG_021`, wenn `expected_server` nicht mehr der
/// aktive Server ist.
pub fn set_expecting(
    paths: &Paths,
    key: &str,
    value: &str,
    expected_server: &str,
) -> Result<v1::ConfigSnapshot, Diagnostic> {
    if key.starts_with(SANDBOX_ENV_PREFIX) {
        let name = accepted(key, value)?;
        humanitl_config::edit::set_sandbox_env(&paths.config_path(), name, value)?;
    } else if FORM_KEYS.contains(&key) {
        // Endpunkt und Modelle sind die Ansicht des aktiven Servers: Sie
        // ändern sich nie, während `LlmKey` einen Schlüssel ablegt (HUM-289).
        let _order = humanitl_config::servers::is_server_setting(key)
            .then(|| humanitl_config::servers::lock(paths))
            .transpose()?;
        set_form_value(paths, key, value, expected_server.trim())?;
    } else {
        return Err(refused(&format!(
            "{} is not one of the settings of the form",
            shown(key)
        )));
    }
    Ok(snapshot(paths))
}

/// Liest und prüft einen Wert des Formulars nach dem Schema, ohne zu
/// schreiben.
///
/// Der Fake (`humanitld --fake`) nimmt denselben Weg und hält das Ergebnis im
/// Speicher; so antworten beide Dienste auf denselben falschen Wert mit
/// demselben Befund.
///
/// # Errors
///
/// `CONFIG_014`, wenn `key` keine Einstellung des Formulars ist, und
/// `CONFIG_003` für einen Text, der kein Wert des Feldes ist oder außerhalb
/// seines Bereichs liegt.
pub fn form_value(
    key: &str,
    text: &str,
) -> Result<(&'static schema::Field, serde_json::Value), Diagnostic> {
    if !FORM_KEYS.contains(&key) {
        return Err(refused(&format!(
            "{} is not one of the settings of the form",
            shown(key)
        )));
    }
    let field = schema::field(key)
        .ok_or_else(|| refused(&format!("{key} is not in the schema of this daemon")))?;
    let value = parse_value(field, text)?;
    check_value(field, &value)?;
    Ok((field, value))
}

/// Schreibt eine Einstellung des Formulars.
fn set_form_value(
    paths: &Paths,
    key: &str,
    text: &str,
    expected_server: &str,
) -> Result<(), Diagnostic> {
    let (field, value) = form_value(key, text)?;
    let toml_value = to_toml(field, &value)?;
    let file = paths.config_path();
    let segments: Vec<String> = key.split('.').map(ToOwned::to_owned).collect();
    if expected_server.is_empty() {
        // Gibt es gespeicherte Server, nennt ein Client, der die Ansicht des
        // aktiven schreibt, ihn auch (Review): Ohne Namen ließe sich nicht
        // prüfen, ob der Wert für diesen Server getippt wurde.
        if humanitl_config::servers::is_server_setting(key)
            && humanitl_config::servers::has_servers(&crate::llm_servers::read_table(paths)?)
        {
            return Err(humanitl_core::Diagnostic::builder(
                humanitl_core::diagnostics::codes::CONFIG_021,
                Severity::Error,
            )
            .why(format!(
                "{key} was not written: saved model servers exist, and the request names none; \
                 a client writes the active server's view only together with its name \
                 (SetConfigRequest.expected_server)"
            ))
            .fix(humanitl_core::FixAction::CopyCommand(
                "humanitl llm server ls".to_owned(),
            ))
            .build());
        }
        humanitl_config::edit::set_value(&file, &segments, toml_value.as_ref(), |candidate| {
            probe(paths, &file, candidate, key)
        })?;
        return Ok(());
    }
    // Unter der Sperre der Datei: Erst wenn der erwartete Server noch aktiv
    // ist, geht der Wert in die flachen Schlüssel und in seinen Eintrag.
    humanitl_config::edit::edit_with(
        &file,
        |old| {
            let active = humanitl_config::servers::active_name(old).unwrap_or_default();
            if active != expected_server {
                return Err(humanitl_core::Diagnostic::builder(
                    humanitl_core::diagnostics::codes::CONFIG_021,
                    Severity::Error,
                )
                .why(format!(
                    "{key} was not written: it was entered for the server {expected_server}, \
                     and the active server is {} now",
                    if active.is_empty() {
                        "none"
                    } else {
                        active.as_str()
                    }
                ))
                .build());
            }
            let mut edits = vec![humanitl_config::edit::Edit::new(
                segments.clone(),
                toml_value.clone(),
            )];
            edits.extend(humanitl_config::servers::mirror_of(
                old,
                &segments,
                toml_value.as_ref(),
            ));
            Ok(edits)
        },
        |candidate| probe(paths, &file, candidate, key),
    )?;
    Ok(())
}

/// Lädt die Konfiguration so, wie der nächste Start sie lädt — mit der
/// Nebendatei an der Stelle von `config.toml` — und entscheidet, ob der Wert
/// geschrieben werden darf.
///
/// Geschrieben wird, wenn die Konfiguration mit dem neuen Wert lädt, oder wenn
/// sie schon vorher mit genau demselben Befund nicht lud und der Befund den
/// gesetzten Schlüssel nicht nennt: Ein alter Fehler an anderer Stelle soll
/// das Formular nicht sperren, ein Wert, der selbst falsch ist, wird nie
/// geschrieben. Die feinere Zählung über alle Befunde einer Ebenenfolge hat
/// `humanitl config set` (HUM-070); das Formular schreibt acht Schlüssel mit
/// festen Typen, und für sie genügt der erste Befund.
///
/// # Errors
///
/// Der Befund der Ladung, mit dem Pfad der Nebendatei durch den von
/// `config.toml` ersetzt.
pub(crate) fn probe(
    paths: &Paths,
    file: &Path,
    candidate: &Path,
    key: &str,
) -> Result<(), Diagnostic> {
    let Err(after) = load_with(paths, Some(candidate.to_path_buf())) else {
        return Ok(());
    };
    let candidate_text = candidate.display().to_string();
    let file_text = file.display().to_string();
    let after_why = after.why.replace(&candidate_text, &file_text);
    let before = load_with(paths, file.is_file().then(|| file.to_path_buf()));
    if let Err(before) = before
        && before.code == after.code
        && before.why == after_why
        && !after_why.contains(key)
    {
        return Ok(());
    }
    let mut refusal = after;
    refusal.why = after_why;
    Err(refusal)
}

/// Die Konfiguration ohne Profilwunsch mit dieser Datei als globaler.
fn load_with(paths: &Paths, global: Option<PathBuf>) -> Result<Resolved, Diagnostic> {
    let sources = humanitl_config::sources_with_global(
        &ProfileSelection::default(),
        None,
        paths.env(),
        &[],
        global,
    )?;
    humanitl_config::load(&sources)
}

/// Der Name der Variable, wenn der Auftrag unter `sandbox.env` angenommen
/// wird.
///
/// # Errors
///
/// `CONFIG_014`, wenn der Schlüssel nicht `sandbox.env.<NAME>` ist, `NAME` nicht
/// aus Großbuchstaben, Ziffern und `_` besteht oder mit einer Ziffer beginnt,
/// `NAME` den dynamischen Loader steuert, oder wenn der Wert nicht das
/// Zertifikat der Sandbox ist.
pub fn accepted<'a>(key: &'a str, value: &str) -> Result<&'a str, Diagnostic> {
    let Some(name) = key.strip_prefix(SANDBOX_ENV_PREFIX) else {
        return Err(refused(&format!(
            "{} is not an environment variable of the sandbox",
            shown(key)
        )));
    };
    if !is_variable_name(name) {
        return Err(refused(&format!(
            "{} is not a name this call writes: upper-case letters, digits and _, not starting \
             with a digit",
            shown(name)
        )));
    }
    if humanitl_config::is_loader_key(name) {
        return Err(refused(&format!("{name} steers the dynamic loader")));
    }
    if value != CA_CERT_DST {
        return Err(refused(&format!(
            "the value for {name} is not {CA_CERT_DST}"
        )));
    }
    Ok(name)
}

/// Ob `name` ein Variablenname ist, wie ihn die CA-Variablen tragen.
fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_uppercase() || first == '_')
        && chars.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// Der Anfang eines Textes, den ein Client geschickt hat, gekürzt für den
/// Befund.
fn shown(text: &str) -> String {
    if text.chars().count() <= SHOWN_KEY_CHARS {
        return text.to_owned();
    }
    let head: String = text.chars().take(SHOWN_KEY_CHARS).collect();
    format!("{head}…")
}

/// `CONFIG_014` mit dem Grund und dem, was angenommen würde.
fn refused(reason: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_014, Severity::Error)
        .why(format!(
            "SetConfig did not write this: {reason}. It writes the settings of the form ({}) \
             and sandbox.env.NAME set to {CA_CERT_DST}, the certificate Humanitl mounts in the \
             sandbox. Anything else goes into config.toml by hand (humanitl config edit).",
            FORM_KEYS.join(", ")
        ))
        .build()
}

/// Die Konfiguration ohne Profilwunsch, wie eine Sitzung ohne Wunsch sie
/// bekäme, samt dem Pfad der Datei, die `SetConfig` schreibt.
///
/// Scheitert die Auflösung — ein Profil, das nicht mehr parst, ein anderer
/// Wert außerhalb seines Bereichs —, steht ihr Befund in `diagnostics`, und
/// `toml` bleibt leer. Nach einem `SetConfig` ist trotzdem geschrieben; eine
/// Antwort, die das als Fehler meldete, sagte dem Menschen, sein Klick sei
/// verloren.
#[must_use]
pub fn snapshot(paths: &Paths) -> v1::ConfigSnapshot {
    let resolved = humanitl_config::resolve(&ProfileSelection::default(), None, paths.env(), &[]);
    let mut snapshot =
        match resolved.and_then(|resolved| convert::config_snapshot_to_proto(&resolved, false)) {
            Ok(snapshot) => snapshot,
            Err(diagnostic) => v1::ConfigSnapshot {
                diagnostics: vec![convert::diagnostic_to_proto(&diagnostic)],
                ..v1::ConfigSnapshot::default()
            },
        };
    snapshot.config_path = paths.config_path().display().to_string();
    snapshot
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::Env;

    use super::*;

    const KEY: &str = "sandbox.env.CURL_CA_BUNDLE";

    fn refusal(key: &str, value: &str) -> Diagnostic {
        accepted(key, value).expect_err("must be refused")
    }

    /// Pfade unter einem eigenen Verzeichnis: Kein Test fasst die
    /// Konfiguration des Menschen an.
    fn paths_in(dir: &Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    #[test]
    fn the_ca_variable_with_the_sandbox_certificate_is_accepted() {
        assert_eq!(accepted(KEY, CA_CERT_DST).unwrap(), "CURL_CA_BUNDLE");
        assert_eq!(
            accepted("sandbox.env.NODE_EXTRA_CA_CERTS", CA_CERT_DST).unwrap(),
            "NODE_EXTRA_CA_CERTS"
        );
    }

    #[test]
    fn another_value_is_config_014() {
        let refused = refusal(KEY, "/tmp/evil.crt");
        assert_eq!(refused.code, CONFIG_014);
        assert!(
            refused.why.contains("is not /etc/humanitl/ca.crt"),
            "{}",
            refused.why
        );
    }

    #[test]
    fn a_key_outside_sandbox_env_is_config_014() {
        for key in [
            "hold.timeout_secs",
            "sandbox.env",
            "sandbox.envX.CURL_CA_BUNDLE",
            "CURL_CA_BUNDLE",
        ] {
            assert_eq!(refusal(key, CA_CERT_DST).code, CONFIG_014, "{key}");
        }
    }

    #[test]
    fn a_name_that_is_not_a_plain_variable_is_config_014() {
        for name in ["", "curl_ca_bundle", "1ST", "A.B", "A B", "A=B", "Ä"] {
            let key = format!("{SANDBOX_ENV_PREFIX}{name}");
            assert_eq!(refusal(&key, CA_CERT_DST).code, CONFIG_014, "{key:?}");
        }
    }

    #[test]
    fn a_loader_variable_is_config_014_even_with_the_certificate() {
        let refused = refusal("sandbox.env.LD_PRELOAD", CA_CERT_DST);
        assert_eq!(refused.code, CONFIG_014);
        assert!(refused.why.contains("dynamic loader"), "{}", refused.why);
    }

    #[test]
    fn a_long_key_is_cut_in_the_finding() {
        let key = "x".repeat(10_000);
        let refused = refusal(&key, CA_CERT_DST);
        assert!(refused.why.len() < 1_000, "{} bytes", refused.why.len());
    }

    /// Jede Einstellung des Formulars landet in `config.toml`, Kommentare und
    /// Reihenfolge bleiben, und die Antwort trägt den Wert (HUM-069).
    #[test]
    fn a_form_setting_is_written_and_the_comment_stays() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let work = dir.path().join("project");
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(
            paths.config_path(),
            "# mein Kommentar\n[hold]\ntimeout_secs = 300 # fünf Minuten\n",
        )
        .unwrap();

        let work_text = work.display().to_string();
        for (key, value) in [
            ("hold.timeout_secs", "600"),
            ("hold.ask_mode", "none"),
            ("ui.language", "de"),
            ("ui.theme", "light"),
            ("ui.notifications", "false"),
            ("sandbox.work_dir", work_text.as_str()),
            ("llm.endpoint", "http://192.168.1.20:11434/"),
            ("llm.models", r#"["qwen2.5-coder:14b"]"#),
            ("tls.trust_system_store", "false"),
        ] {
            set(&paths, key, value).unwrap_or_else(|d| panic!("{key}: {}", d.why));
        }
        let text = std::fs::read_to_string(paths.config_path()).unwrap();
        assert!(
            text.starts_with("# mein Kommentar\n[hold]\ntimeout_secs = 600 # fünf Minuten"),
            "{text}"
        );
        let snapshot = snapshot(&paths);
        let values: serde_json::Value = serde_json::from_str(&snapshot.values_json).unwrap();
        assert_eq!(values["hold"]["timeout_secs"], 600);
        assert_eq!(values["hold"]["ask_mode"], "none");
        assert_eq!(values["ui"]["language"], "de");
        assert_eq!(values["ui"]["theme"], "light");
        assert_eq!(values["ui"]["notifications"], false);
        assert_eq!(values["sandbox"]["work_dir"], work_text);
        assert_eq!(values["llm"]["endpoint"], "http://192.168.1.20:11434/");
        assert_eq!(values["llm"]["models"][0], "qwen2.5-coder:14b");
        assert_eq!(values["tls"]["trust_system_store"], false);
        assert_eq!(
            snapshot.config_path,
            paths.config_path().display().to_string()
        );
    }

    /// Ein Wert außerhalb des Bereichs und ein Wert, den die Prüfung der
    /// Konfiguration ablehnt, kommen nicht in die Datei (HUM-069).
    #[test]
    fn a_wrong_value_is_refused_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        for (key, value) in [
            ("hold.ask_mode", "sometimes"),
            ("ui.theme", "pink"),
            ("hold.timeout_secs", "0"),
            ("hold.timeout_secs", "soon"),
            ("llm.endpoint", "ftp://model.lan/"),
            ("sandbox.work_dir", "relative/dir"),
            ("tls.trust_system_store", "maybe"),
        ] {
            let refused = set(&paths, key, value).expect_err(key);
            assert_eq!(
                refused.code.as_str(),
                "CONFIG_003",
                "{key} = {value}: {}",
                refused.why
            );
        }
        assert!(!paths.config_path().exists(), "nothing was written");
    }

    /// Ein Schlüssel außerhalb des Formulars bleibt `CONFIG_014`, auch der
    /// Verweis auf den Zugangsschlüssel: Den schreibt nur `LlmKey`.
    #[test]
    fn a_key_outside_the_form_is_config_014() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        for key in [
            "llm.api_key_ref",
            "sandbox.profile",
            "agent.command",
            "audit.anchor_every",
        ] {
            let refused = set(&paths, key, "x").expect_err(key);
            assert_eq!(refused.code, CONFIG_014, "{key}");
        }
        assert!(!paths.config_path().exists());
    }
}
