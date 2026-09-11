//! `SetConfig`, so schmal wie sein Anlass (HUM-151).
//!
//! Der RPC im Ganzen wartet auf den Einstellungen-Bildschirm (HUM-069). Bis
//! dahin nimmt er genau eine Art Auftrag an, die ein `TLS_001` vorschlägt: eine
//! Variable unter `sandbox.env`, deren Wert das Zertifikat ist, das Humanitl in
//! der Sandbox einhängt. Ein Client ändert damit keine Einhängung, keinen
//! Befehl und kein Ziel des Verkehrs — warum das nicht in seine Hand gehört,
//! steht an [`crate::session::check_override_key`]. Er sagt nur einem Werkzeug,
//! wo das Zertifikat liegt, das ohnehin dort liegt.
//!
//! Die Datei ändert [`humanitl_config::edit::set_sandbox_env`]; hier steht, was
//! angenommen wird, und woraus die Antwort besteht.

use humanitl_config::{Paths, ProfileSelection};
use humanitl_core::diagnostics::codes::CONFIG_014;
use humanitl_core::{Diagnostic, Severity};
use humanitl_sandbox::profile::CA_CERT_DST;

use crate::{convert, v1};

/// Der Anfang jedes Schlüssels, den `SetConfig` heute annimmt.
pub const SANDBOX_ENV_PREFIX: &str = "sandbox.env.";

/// So viele Zeichen eines abgelehnten Schlüssels stehen im Befund. Der Text
/// reist im Status mit, und ein Client bestimmt die Länge des Schlüssels.
const SHOWN_KEY_CHARS: usize = 80;

/// Setzt die Variable und antwortet mit der Konfiguration, wie sie danach gilt.
///
/// Blockiert, weil es eine Datei liest und schreibt; der Dienst ruft es in
/// [`tokio::task::spawn_blocking`].
///
/// # Errors
///
/// `CONFIG_014` für einen Auftrag, den [`accepted`] ablehnt, und die Befunde von
/// [`humanitl_config::edit::set_sandbox_env`], wenn die Datei nicht geändert
/// wurde. Geschrieben ist in keinem der Fälle etwas.
pub fn set(paths: &Paths, key: &str, value: &str) -> Result<v1::ConfigSnapshot, Diagnostic> {
    let name = accepted(key, value)?;
    humanitl_config::edit::set_sandbox_env(&paths.config_path(), name, value)?;
    Ok(snapshot(paths))
}

/// Der Name der Variable, wenn der Auftrag angenommen wird.
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
            "SetConfig did not write this: {reason}. Until the settings screen arrives (HUM-069) \
             it writes one kind of setting to config.toml: sandbox.env.NAME set to \
             {CA_CERT_DST}, the certificate Humanitl mounts in the sandbox. Anything else goes \
             into config.toml by hand."
        ))
        .build()
}

/// Die Konfiguration ohne Profilwunsch, wie eine Sitzung ohne Wunsch sie
/// bekäme.
///
/// Scheitert die Auflösung — ein Profil, das nicht mehr parst, ein anderer
/// Wert außerhalb seines Bereichs —, steht ihr Befund in `diagnostics`, und
/// `toml` bleibt leer. Geschrieben ist dann trotzdem; eine Antwort, die das
/// als Fehler meldete, sagte dem Menschen, sein Klick sei verloren.
fn snapshot(paths: &Paths) -> v1::ConfigSnapshot {
    let resolved = humanitl_config::resolve(&ProfileSelection::default(), None, paths.env(), &[]);
    match resolved.and_then(|resolved| convert::config_snapshot_to_proto(&resolved, false)) {
        Ok(snapshot) => snapshot,
        Err(diagnostic) => v1::ConfigSnapshot {
            diagnostics: vec![convert::diagnostic_to_proto(&diagnostic)],
            ..v1::ConfigSnapshot::default()
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const KEY: &str = "sandbox.env.CURL_CA_BUNDLE";

    fn refusal(key: &str, value: &str) -> Diagnostic {
        accepted(key, value).expect_err("must be refused")
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
}
